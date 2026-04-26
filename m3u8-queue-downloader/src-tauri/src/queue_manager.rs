use crate::app_error::{AppError, AppResult};
use crate::models::{AddTaskPayload, QueueState, Task, TaskStatus};
use crate::persistence::Persistence;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub enum TaskFailureTransition {
    RetryScheduled,
    Terminal(Task),
}

pub struct QueueManager {
    state: Arc<Mutex<QueueState>>,
    persistence_path: PathBuf,
    shutting_down: Arc<Mutex<bool>>,
}

impl QueueManager {
    pub fn new(persistence_path: PathBuf) -> Self {
        let state = Persistence::load(&persistence_path).unwrap_or_default();
        Self {
            state: Arc::new(Mutex::new(state)),
            persistence_path,
            shutting_down: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn add_task(&self, payload: AddTaskPayload) -> AppResult<(Task, bool)> {
        let task = Task::new(payload.url, payload.save_name, payload.headers);
        let mut state = self.state.lock().await;
        state.tasks.push(task.clone());
        let should_schedule = state.is_running && state.current_task_id.is_none();
        self.persist(&state)?;
        drop(state);
        Ok((task, should_schedule))
    }

    pub async fn remove_task(&self, id: &str) -> AppResult<()> {
        let mut state = self.state.lock().await;
        let task = state.tasks.iter().find(|t| t.id == id);
        match task {
            Some(t) => {
                if t.status == TaskStatus::Waiting || t.status == TaskStatus::Failed {
                    state.tasks.retain(|t| t.id != id);
                    self.persist(&state)?;
                    Ok(())
                } else {
                    Err(AppError::InvalidTaskStatus {
                        action: "remove",
                        id: id.to_string(),
                        status: format!("{:?}", t.status),
                    })
                }
            }
            None => Err(AppError::TaskNotFound { id: id.to_string() }),
        }
    }

    pub async fn retry_task(&self, id: &str) -> AppResult<Task> {
        let mut state = self.state.lock().await;
        let task = state.tasks.iter_mut().find(|t| t.id == id);
        match task {
            Some(t) => {
                if t.status == TaskStatus::Failed {
                    t.status = TaskStatus::Waiting;
                    t.progress = 0.0;
                    t.speed = String::new();
                    t.threads = String::new();
                    t.error_message = None;
                    let task = t.clone();
                    self.persist(&state)?;
                    Ok(task)
                } else {
                    Err(AppError::InvalidTaskStatus {
                        action: "retry",
                        id: id.to_string(),
                        status: format!("{:?}", t.status),
                    })
                }
            }
            None => Err(AppError::TaskNotFound { id: id.to_string() }),
        }
    }

    pub async fn add_history_retry_task(&self, task: &Task) -> AppResult<(Task, bool)> {
        let payload = AddTaskPayload {
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
        };

        self.add_task(payload).await
    }

    pub async fn reorder_tasks(&self, task_ids: Vec<String>) -> AppResult<()> {
        let mut state = self.state.lock().await;

        let original_tasks = std::mem::take(&mut state.tasks);
        let mut waiting_tasks: Vec<Task> = original_tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Waiting)
            .cloned()
            .collect();
        let non_waiting: Vec<Task> = original_tasks
            .into_iter()
            .filter(|task| task.status != TaskStatus::Waiting)
            .collect();

        let mut reordered_waiting = Vec::with_capacity(waiting_tasks.len());
        for id in &task_ids {
            if let Some(position) = waiting_tasks.iter().position(|task| &task.id == id) {
                reordered_waiting.push(waiting_tasks.remove(position));
            }
        }
        reordered_waiting.extend(waiting_tasks);

        state.tasks = non_waiting
            .into_iter()
            .chain(reordered_waiting.into_iter())
            .collect();
        self.persist(&state)?;
        Ok(())
    }

    pub async fn get_state(&self) -> QueueState {
        self.state.lock().await.clone()
    }

    pub async fn schedule_next(&self) -> AppResult<Option<Task>> {
        if self.is_shutting_down().await {
            return Ok(None);
        }

        let mut state = self.state.lock().await;

        if state.current_task_id.is_some() {
            return Ok(None);
        }

        if !state.is_running {
            return Ok(None);
        }

        let next_task = state
            .tasks
            .iter()
            .find(|t| t.status == TaskStatus::Waiting)
            .cloned();

        if let Some(ref task) = next_task {
            if let Some(t) = state.tasks.iter_mut().find(|t| t.id == task.id) {
                t.status = TaskStatus::Downloading;
            }
            state.current_task_id = Some(task.id.clone());
            self.persist(&state)?;
        }

        Ok(next_task.map(|mut t| {
            t.status = TaskStatus::Downloading;
            t
        }))
    }

    pub async fn snapshot_task_completion(&self, id: &str, output_path: &str) -> Option<Task> {
        let state = self.state.lock().await;
        let mut task = state.tasks.iter().find(|t| t.id == id)?.clone();
        task.status = TaskStatus::Completed;
        task.progress = 1.0;
        task.output_path = Some(output_path.to_string());
        Some(task)
    }

    pub async fn finalize_task_completion(&self, id: &str) -> AppResult<bool> {
        let mut state = self.state.lock().await;
        let removed = if let Some(position) = state.tasks.iter().position(|t| t.id == id) {
            state.tasks.remove(position);
            true
        } else {
            false
        };
        if state.current_task_id.as_deref() == Some(id) {
            state.current_task_id = None;
        }
        self.persist(&state)?;
        Ok(removed)
    }

    pub async fn prepare_task_failure(
        &self,
        id: &str,
        error_message: &str,
    ) -> AppResult<Option<TaskFailureTransition>> {
        if self.is_shutting_down().await {
            return Ok(None);
        }

        let mut state = self.state.lock().await;
        let transition = if let Some(position) = state.tasks.iter().position(|t| t.id == id) {
            let t = &mut state.tasks[position];
            if t.retry_count < 2 {
                t.retry_count += 1;
                t.status = TaskStatus::Waiting;
                t.progress = 0.0;
                t.speed = String::new();
                t.threads = String::new();
                t.error_message = None;
                Some(TaskFailureTransition::RetryScheduled)
            } else {
                let mut task = t.clone();
                task.status = TaskStatus::Failed;
                task.error_message = Some(error_message.to_string());
                Some(TaskFailureTransition::Terminal(task))
            }
        } else {
            None
        };
        if !matches!(transition, Some(TaskFailureTransition::Terminal(_)))
            && state.current_task_id.as_deref() == Some(id)
        {
            state.current_task_id = None;
        }
        self.persist(&state)?;
        Ok(transition)
    }

    pub async fn finalize_terminal_failure(&self, id: &str) -> AppResult<bool> {
        let mut state = self.state.lock().await;
        let removed = if let Some(position) = state.tasks.iter().position(|t| t.id == id) {
            state.tasks.remove(position);
            true
        } else {
            false
        };
        if state.current_task_id.as_deref() == Some(id) {
            state.current_task_id = None;
        }
        self.persist(&state)?;
        Ok(removed)
    }

    pub async fn update_task_progress(
        &self,
        id: &str,
        progress: Option<f32>,
        speed: Option<String>,
        threads: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        if let Some(t) = state.tasks.iter_mut().find(|t| t.id == id) {
            if let Some(progress) = progress {
                t.progress = progress.clamp(0.0, 1.0);
            }
            if let Some(speed) = speed.filter(|value| !value.is_empty()) {
                t.speed = speed;
            }
            if let Some(threads) = threads.filter(|value| !value.is_empty()) {
                t.threads = threads;
            }
        }
    }

    pub async fn finish_run_if_idle(&self) -> AppResult<bool> {
        let mut state = self.state.lock().await;
        let has_live_work = state.tasks.iter().any(|task| {
            task.status == TaskStatus::Waiting || task.status == TaskStatus::Downloading
        });

        if state.is_running && !has_live_work && state.current_task_id.is_none() {
            state.is_running = false;
            self.persist(&state)?;
            return Ok(true);
        }

        Ok(false)
    }

    pub async fn has_live_work(&self) -> bool {
        let state = self.state.lock().await;
        state.tasks.iter().any(|task| {
            task.status == TaskStatus::Waiting || task.status == TaskStatus::Downloading
        })
    }

    pub async fn set_running(&self, running: bool) -> AppResult<()> {
        if running {
            self.clear_shutdown_flag().await;
        }

        let mut state = self.state.lock().await;
        state.is_running = running;
        self.persist(&state)
    }

    pub async fn prepare_for_exit(&self) -> AppResult<()> {
        self.mark_shutting_down().await;

        let mut state = self.state.lock().await;
        state.is_running = false;
        state.current_task_id = None;

        for task in &mut state.tasks {
            if task.status == TaskStatus::Downloading {
                task.status = TaskStatus::Waiting;
                task.progress = 0.0;
                task.speed = String::new();
                task.threads = String::new();
            }
        }

        self.persist(&state)
    }

    pub async fn is_shutting_down(&self) -> bool {
        *self.shutting_down.lock().await
    }

    async fn mark_shutting_down(&self) {
        let mut shutting_down = self.shutting_down.lock().await;
        *shutting_down = true;
    }

    async fn clear_shutdown_flag(&self) {
        let mut shutting_down = self.shutting_down.lock().await;
        *shutting_down = false;
    }

    fn persist(&self, state: &QueueState) -> AppResult<()> {
        Persistence::save(state, &self.persistence_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_task_keeps_paused_queue_paused() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        manager
            .set_running(false)
            .await
            .expect("persist running state");
        let payload = AddTaskPayload {
            url: "https://example.com/paused.m3u8".to_string(),
            save_name: None,
            headers: None,
        };

        let (_, should_schedule) = manager.add_task(payload).await.expect("add task");
        let state = manager.get_state().await;

        assert!(!state.is_running);
        assert!(!should_schedule);
    }

    #[tokio::test]
    async fn add_task_requests_schedule_when_queue_is_running_and_idle() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        manager
            .set_running(true)
            .await
            .expect("persist running state");
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };

        let (_, should_schedule) = manager.add_task(payload).await.expect("add task");
        let state = manager.get_state().await;

        assert!(state.is_running);
        assert!(should_schedule);
    }

    #[tokio::test]
    async fn add_task_reports_persistence_failure() {
        let path = std::env::temp_dir().join(format!("queue-state-dir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create blocking directory");
        let manager = QueueManager::new(path.clone());
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };

        let result = manager.add_task(payload).await;

        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn prepare_for_exit_resets_downloading_state() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        manager
            .set_running(true)
            .await
            .expect("persist running state");
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = manager.add_task(payload).await.expect("add task");
        manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("schedule first task");
        manager
            .update_task_progress(&task.id, Some(0.5), Some("1 MB/s".to_string()), None)
            .await;

        manager.prepare_for_exit().await.expect("prepare exit");

        let state = manager.get_state().await;
        let prepared = state
            .tasks
            .iter()
            .find(|t| t.id == task.id)
            .expect("task exists");
        assert!(!state.is_running);
        assert!(state.current_task_id.is_none());
        assert_eq!(prepared.status, TaskStatus::Waiting);
        assert_eq!(prepared.progress, 0.0);
        assert!(prepared.speed.is_empty());
    }

    #[tokio::test]
    async fn prepare_for_exit_ignores_late_child_failure() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        manager
            .set_running(true)
            .await
            .expect("persist running state");
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = manager.add_task(payload).await.expect("add task");
        manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("schedule first task");

        manager.prepare_for_exit().await.expect("prepare exit");
        let transition = manager
            .prepare_task_failure(&task.id, "killed during shutdown")
            .await
            .expect("prepare task failure");

        let state = manager.get_state().await;
        let prepared = state
            .tasks
            .iter()
            .find(|t| t.id == task.id)
            .expect("task exists");
        assert!(transition.is_none());
        assert!(!state.is_running);
        assert!(state.current_task_id.is_none());
        assert_eq!(prepared.status, TaskStatus::Waiting);
        assert_eq!(prepared.retry_count, 0);
    }

    #[tokio::test]
    async fn reorder_waiting_tasks_persists_across_reload() {
        let path = std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4()));
        let manager = QueueManager::new(path.clone());

        let (first, _) = manager
            .add_task(AddTaskPayload {
                url: "https://example.com/1.m3u8".to_string(),
                save_name: Some("first".to_string()),
                headers: None,
            })
            .await
            .expect("add first task");
        let (second, _) = manager
            .add_task(AddTaskPayload {
                url: "https://example.com/2.m3u8".to_string(),
                save_name: Some("second".to_string()),
                headers: None,
            })
            .await
            .expect("add second task");
        let (third, _) = manager
            .add_task(AddTaskPayload {
                url: "https://example.com/3.m3u8".to_string(),
                save_name: Some("third".to_string()),
                headers: None,
            })
            .await
            .expect("add third task");

        manager
            .set_running(true)
            .await
            .expect("persist running state");
        manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("schedule first task");
        manager
            .reorder_tasks(vec![third.id.clone(), second.id.clone()])
            .await
            .expect("reorder waiting tasks");

        let state = manager.get_state().await;
        let ids: Vec<_> = state.tasks.iter().map(|task| task.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![first.id.as_str(), third.id.as_str(), second.id.as_str()]
        );

        let reloaded = QueueManager::new(path.clone());
        let reloaded_state = reloaded.get_state().await;
        let reloaded_ids: Vec<_> = reloaded_state
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect();
        assert_eq!(reloaded_ids, ids);

        std::fs::remove_file(path).expect("cleanup queue state");
    }
}
