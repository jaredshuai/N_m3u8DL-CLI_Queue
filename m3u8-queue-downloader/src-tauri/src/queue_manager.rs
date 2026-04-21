use crate::models::{AddTaskPayload, QueueState, Task, TaskStatus};
use crate::persistence::Persistence;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const MAX_LOG_LINES: usize = 500;

/// Manages the download queue state with interior mutability
pub struct QueueManager {
    state: Arc<Mutex<QueueState>>,
    persistence_path: PathBuf,
}

impl QueueManager {
    pub fn new(persistence_path: PathBuf) -> Self {
        // Try to load persisted state
        let state = Persistence::load(&persistence_path).unwrap_or_default();
        Self {
            state: Arc::new(Mutex::new(state)),
            persistence_path,
        }
    }

    /// Add a new task to the queue. Returns (task, should_schedule).
    pub async fn add_task(&self, payload: AddTaskPayload) -> (Task, bool) {
        let task = Task::new(payload.url, payload.save_name, payload.headers);
        let mut state = self.state.lock().await;
        state.tasks.push(task.clone());
        let should_schedule = state.is_running && state.current_task_id.is_none();
        self.persist(&state);
        drop(state);
        (task, should_schedule)
    }

    /// Remove a task from the queue (only if Waiting or Failed)
    pub async fn remove_task(&self, id: &str) -> Result<(), String> {
        let mut state = self.state.lock().await;
        let task = state.tasks.iter().find(|t| t.id == id);
        match task {
            Some(t) => {
                if t.status == TaskStatus::Waiting || t.status == TaskStatus::Failed {
                    state.tasks.retain(|t| t.id != id);
                    self.persist(&state);
                    Ok(())
                } else {
                    Err(format!(
                        "Cannot remove task {} with status {:?}",
                        id, t.status
                    ))
                }
            }
            None => Err(format!("Task {} not found", id)),
        }
    }

    /// Retry a failed task - reset to Waiting with retry_count preserved
    pub async fn retry_task(&self, id: &str) -> Result<Task, String> {
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
                    t.log_lines.clear();
                    // retry_count is preserved (already incremented from previous failure)
                    let task = t.clone();
                    self.persist(&state);
                    Ok(task)
                } else {
                    Err(format!(
                        "Can only retry Failed tasks, current status: {:?}",
                        t.status
                    ))
                }
            }
            None => Err(format!("Task {} not found", id)),
        }
    }

    pub async fn add_history_retry_task(&self, task: &Task) -> (Task, bool) {
        let payload = AddTaskPayload {
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
        };

        self.add_task(payload).await
    }

    /// Reorder waiting tasks by the given ID order, preserving non-waiting tasks' positions
    pub async fn reorder_tasks(&self, task_ids: Vec<String>) -> Result<(), String> {
        let mut state = self.state.lock().await;

        // Separate waiting and non-waiting tasks
        let mut non_waiting: Vec<Task> = Vec::new();
        let mut waiting_map: std::collections::HashMap<String, Task> =
            std::collections::HashMap::new();

        for task in state.tasks.drain(..) {
            if task.status == TaskStatus::Waiting {
                waiting_map.insert(task.id.clone(), task);
            } else {
                non_waiting.push(task);
            }
        }

        // Rebuild: non-waiting first, then waiting in the specified order
        let mut result: Vec<Task> = non_waiting;
        for id in &task_ids {
            if let Some(task) = waiting_map.remove(id) {
                result.push(task);
            }
        }
        // Append any waiting tasks not in the provided order
        result.extend(waiting_map.into_values());

        state.tasks = result;
        self.persist(&state);
        Ok(())
    }

    /// Get a clone of the current queue state
    pub async fn get_state(&self) -> QueueState {
        self.state.lock().await.clone()
    }

    /// Check if schedule_next should be called and return the next waiting task
    pub async fn schedule_next(&self) -> Option<Task> {
        let mut state = self.state.lock().await;

        // If already processing a task, do nothing
        if state.current_task_id.is_some() {
            return None;
        }

        // If queue is not running, do nothing
        if !state.is_running {
            return None;
        }

        // Find the first Waiting task
        let next_task = state
            .tasks
            .iter()
            .find(|t| t.status == TaskStatus::Waiting)
            .cloned();

        if let Some(ref task) = next_task {
            // Mark task as Downloading and set as current
            if let Some(t) = state.tasks.iter_mut().find(|t| t.id == task.id) {
                t.status = TaskStatus::Downloading;
            }
            state.current_task_id = Some(task.id.clone());
            self.persist(&state);
        }

        // Return the task with updated Downloading status
        next_task.map(|mut t| {
            t.status = TaskStatus::Downloading;
            t
        })
    }

    /// Mark a task as completed
    pub async fn on_task_completed(&self, id: &str, output_path: &str) -> Option<Task> {
        let mut state = self.state.lock().await;
        let completed_task = if let Some(position) = state.tasks.iter().position(|t| t.id == id) {
            let mut task = state.tasks.remove(position);
            task.status = TaskStatus::Completed;
            task.progress = 1.0;
            task.output_path = Some(output_path.to_string());
            Some(task)
        } else {
            None
        };
        state.current_task_id = None;
        self.persist(&state);
        completed_task
    }

    /// Reset a task back to Waiting when queue is paused (no retry_count increment)
    pub async fn on_task_paused(&self, id: &str) {
        let mut state = self.state.lock().await;
        if let Some(t) = state.tasks.iter_mut().find(|t| t.id == id) {
            t.status = TaskStatus::Waiting;
            t.progress = 0.0;
            t.speed = String::new();
            t.threads = String::new();
            t.log_lines.clear();
        }
        state.current_task_id = None;
        self.persist(&state);
    }

    /// Mark a task as failed, handling retry logic
    pub async fn on_task_failed(&self, id: &str, error_message: &str) -> Option<Task> {
        let mut state = self.state.lock().await;
        let terminal_failure = if let Some(position) = state.tasks.iter().position(|t| t.id == id) {
            let t = &mut state.tasks[position];
            if t.retry_count < 2 {
                t.retry_count += 1;
                t.status = TaskStatus::Waiting;
                t.progress = 0.0;
                t.speed = String::new();
                t.threads = String::new();
                t.error_message = None;
                t.log_lines.clear();
                None
            } else {
                let mut task = state.tasks.remove(position);
                task.status = TaskStatus::Failed;
                task.error_message = Some(error_message.to_string());
                Some(task)
            }
        } else {
            None
        };
        state.current_task_id = None;
        self.persist(&state);
        terminal_failure
    }

    /// Update task progress (called from TaskRunner)
    pub async fn update_task_progress(
        &self,
        id: &str,
        progress: f32,
        speed: String,
        threads: String,
    ) {
        let mut state = self.state.lock().await;
        if let Some(t) = state.tasks.iter_mut().find(|t| t.id == id) {
            t.progress = progress;
            t.speed = speed;
            t.threads = threads;
        }
        // Don't persist on every progress update - too frequent
    }

    /// Append a log line to a task (capped at 500 lines)
    pub async fn append_log(&self, id: &str, line: String) {
        let mut state = self.state.lock().await;
        if let Some(t) = state.tasks.iter_mut().find(|t| t.id == id) {
            if t.log_lines.len() >= MAX_LOG_LINES {
                t.log_lines.pop_front();
            }
            t.log_lines.push_back(line);
        }
    }

    /// Set the running flag
    pub async fn set_running(&self, running: bool) {
        let mut state = self.state.lock().await;
        state.is_running = running;
        self.persist(&state);
    }

    /// Get the current task ID
    pub async fn current_task_id(&self) -> Option<String> {
        let state = self.state.lock().await;
        state.current_task_id.clone()
    }

    /// Persist state to disk
    fn persist(&self, state: &QueueState) {
        if let Err(e) = Persistence::save(state, &self.persistence_path) {
            eprintln!("Failed to persist queue state: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_task_keeps_paused_queue_paused() {
        let manager = QueueManager::new(std::env::temp_dir().join(format!(
            "queue-state-{}.json",
            uuid::Uuid::new_v4()
        )));
        manager.set_running(false).await;
        let payload = AddTaskPayload {
            url: "https://example.com/paused.m3u8".to_string(),
            save_name: None,
            headers: None,
        };

        let (_, should_schedule) = manager.add_task(payload).await;
        let state = manager.get_state().await;

        assert!(!state.is_running);
        assert!(!should_schedule);
    }

    #[tokio::test]
    async fn add_task_requests_schedule_when_queue_is_running_and_idle() {
        let manager = QueueManager::new(std::env::temp_dir().join(format!(
            "queue-state-{}.json",
            uuid::Uuid::new_v4()
        )));
        manager.set_running(true).await;
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };

        let (_, should_schedule) = manager.add_task(payload).await;
        let state = manager.get_state().await;

        assert!(state.is_running);
        assert!(should_schedule);
    }

    #[tokio::test]
    async fn append_log_keeps_latest_500_lines() {
        let manager = QueueManager::new(std::env::temp_dir().join(format!(
            "queue-state-{}.json",
            uuid::Uuid::new_v4()
        )));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = manager.add_task(payload).await;

        for i in 0..525 {
            manager.append_log(&task.id, format!("line-{i}")).await;
        }

        let state = manager.get_state().await;
        let task = state
            .tasks
            .iter()
            .find(|candidate| candidate.id == task.id)
            .expect("task exists");

        assert_eq!(task.log_lines.len(), 500);
        assert_eq!(
            task.log_lines.front().map(String::as_str),
            Some("line-25")
        );
        assert_eq!(
            task.log_lines.back().map(String::as_str),
            Some("line-524")
        );
    }
}
