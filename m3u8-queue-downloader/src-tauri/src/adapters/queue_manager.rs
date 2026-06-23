use crate::adapters::queue_repository_mappers::{
    application_add_task_outcome, application_pending_history_clear_outcome,
    application_prepare_task_failure_outcome, application_remove_task_result,
    application_retry_task_result, application_run_finish_outcome,
    application_schedule_next_outcome, application_task_completion_staging_outcome,
    application_terminal_history_staging_outcome, application_update_save_name_result,
    domain_run_status,
};
use crate::adapters::queue_shutdown_gate::QueueShutdownGate;
use crate::adapters::queue_state_store::QueueStateStore;
use crate::application::app_error::AppResult;
use crate::application::queue_repository_outcomes::{
    PrepareTaskFailureOutcome as ApplicationPrepareTaskFailureOutcome,
    QueueRunStatus as ApplicationQueueRunStatus,
};
use crate::application::queue_state_snapshot::QueueStateSnapshot;
use crate::application::task_snapshot::TaskSnapshot;
#[cfg(test)]
use crate::domain::queue::QueueAggregate;
use crate::domain::task::Task;
use crate::ports::queue_repository::{
    QueueMutation, QueueRepositoryFuture, QueueRunLifecycle, QueueStateReader,
};
use std::path::PathBuf;

pub struct QueueManager {
    store: QueueStateStore,
    shutdown_gate: QueueShutdownGate,
}

impl QueueManager {
    pub fn new(persistence_path: PathBuf) -> Self {
        Self {
            store: QueueStateStore::new(persistence_path),
            shutdown_gate: QueueShutdownGate::new(),
        }
    }

    pub async fn add_task(&self, task: Task) -> AppResult<bool> {
        self.store
            .update_and_persist(|state| {
                let outcome = state.add_task(task);
                Ok(application_add_task_outcome(outcome))
            })
            .await
    }

    pub async fn remove_task(&self, id: &str) -> AppResult<()> {
        self.store
            .update_and_persist(|state| {
                application_remove_task_result(id, state.remove_task(id))?;
                Ok(())
            })
            .await
    }

    pub async fn update_save_name(&self, id: &str, save_name: Option<String>) -> AppResult<()> {
        self.store
            .update_and_persist(|state| {
                application_update_save_name_result(id, state.update_save_name(id, save_name))?;
                Ok(())
            })
            .await
    }

    pub async fn retry_task(&self, id: &str) -> AppResult<TaskSnapshot> {
        self.store.reset_runtime_state(id).await;
        self.store
            .update_and_persist(|state| {
                let task = application_retry_task_result(id, state.retry_task(id))?;
                Ok(task)
            })
            .await
    }

    pub async fn reorder_tasks(&self, task_ids: Vec<String>) -> AppResult<()> {
        self.store
            .update_and_persist(|state| {
                state.reorder_waiting_tasks(task_ids);
                Ok(())
            })
            .await
    }

    pub async fn get_queue_state_snapshot(&self) -> QueueStateSnapshot {
        let (tasks, current_task_id, is_running) = self
            .store
            .read(|state| {
                (
                    state.tasks().to_vec(),
                    state.current_task_id().map(str::to_string),
                    state.is_running(),
                )
            })
            .await;
        let runtime_states = self.store.read_runtime_states().await;
        let snapshots: Vec<TaskSnapshot> = tasks
            .iter()
            .map(|task| match runtime_states.get(&task.id) {
                Some(runtime) => TaskSnapshot::from_task_and_runtime(task, runtime),
                None => TaskSnapshot::from(task),
            })
            .collect();
        QueueStateSnapshot {
            tasks: snapshots,
            current_task_id,
            is_running,
        }
    }

    #[cfg(test)]
    pub async fn get_state(&self) -> QueueAggregate {
        self.store.read(Clone::clone).await
    }

    pub async fn schedule_next(&self) -> AppResult<Option<TaskSnapshot>> {
        self.store
            .update_and_persist_when(
                |state| {
                    let outcome = application_schedule_next_outcome(state.schedule_next());
                    Ok(outcome)
                },
                |outcome| outcome.is_some(),
            )
            .await
    }

    pub async fn stage_task_completion(
        &self,
        id: &str,
        output_path: &str,
    ) -> AppResult<Option<TaskSnapshot>> {
        let snapshot_opt = self
            .store
            .update_and_persist_when(
                |state| {
                    let o = application_task_completion_staging_outcome(
                        state.stage_task_completion(id),
                    );
                    Ok(o)
                },
                |o| o.is_some(),
            )
            .await?;

        if let Some(task) = &snapshot_opt {
            self.store
                .update_runtime_state(&task.id, |runtime| {
                    runtime.mark_completed(output_path);
                })
                .await;
        }

        // domain Task 无 output_path 字段，From<Task> 转出的 snapshot 该字段为 None。
        // 这里用真实的 output_path 回填，确保历史记录能持久化下载产物路径。
        let mut snapshot_opt = snapshot_opt;
        if let Some(snapshot) = &mut snapshot_opt {
            if !output_path.is_empty() {
                snapshot.output_path = Some(output_path.to_string());
            }
        }

        Ok(snapshot_opt)
    }

    pub async fn prepare_task_failure(
        &self,
        id: &str,
        error_message: &str,
    ) -> AppResult<ApplicationPrepareTaskFailureOutcome> {
        self.store.reset_runtime_state(id).await;
        self.store
            .update_and_persist(|state| {
                let transition = state.prepare_task_failure(id, error_message);
                Ok(application_prepare_task_failure_outcome(transition))
            })
            .await
    }

    pub async fn pause_after_failure_persistence_error(&self, id: &str, error_message: &str) {
        self.store
            .mutate_memory(|state| {
                state.pause_after_failure_persistence_error(id, error_message);
            })
            .await;
    }

    pub async fn stage_terminal_history_task(&self, id: &str) -> AppResult<Option<TaskSnapshot>> {
        self.store
            .update_and_persist_when(
                |state| {
                    let o = application_terminal_history_staging_outcome(
                        id,
                        state.stage_terminal_history_task(id),
                    )?;
                    Ok(o)
                },
                |o| o.is_some(),
            )
            .await
    }

    pub async fn update_live_task_progress(
        &self,
        id: &str,
        progress: Option<f32>,
        speed: Option<String>,
        threads: Option<String>,
    ) -> bool {
        let speed_for_runtime = speed.clone();
        let threads_for_runtime = threads.clone();

        let exists = self
            .store
            .read(|state| state.tasks().iter().any(|t| t.id == id))
            .await;

        if !exists {
            return false;
        }

        self.store
            .update_runtime_state(id, |runtime| {
                runtime.update_progress(progress, speed_for_runtime, threads_for_runtime);
            })
            .await;

        true
    }

    pub async fn finish_run_if_idle(&self) -> AppResult<bool> {
        self.store
            .update_and_persist_when(
                |state| {
                    let outcome = application_run_finish_outcome(state.finish_run_if_idle());
                    Ok(outcome)
                },
                |outcome| *outcome,
            )
            .await
    }

    pub async fn pending_history_tasks(&self) -> Vec<TaskSnapshot> {
        let tasks = self
            .store
            .read(|state| state.pending_history_snapshot())
            .await;
        let runtime_states = self.store.read_runtime_states().await;
        tasks
            .into_iter()
            .map(|task| match runtime_states.get(&task.id) {
                Some(runtime) => TaskSnapshot::from_task_and_runtime(&task, runtime),
                None => TaskSnapshot::from(&task),
            })
            .collect()
    }

    pub async fn clear_pending_history_task(&self, id: &str) -> AppResult<bool> {
        self.store
            .update_and_persist_when(
                |state| {
                    let o = application_pending_history_clear_outcome(
                        state.clear_pending_history_task(id),
                    );
                    Ok(o)
                },
                |o| *o,
            )
            .await
    }

    pub async fn live_work_status(&self) -> bool {
        self.store.read(|state| state.has_live_work()).await
    }

    pub async fn set_run_status(&self, status: ApplicationQueueRunStatus) -> AppResult<()> {
        self.store
            .update_and_persist(|state| {
                state.set_run_status(domain_run_status(status));
                Ok(())
            })
            .await?;

        if status == ApplicationQueueRunStatus::Running {
            self.shutdown_gate.clear_shutdown_flag().await;
        }

        Ok(())
    }

    pub async fn prepare_for_exit(&self) -> AppResult<()> {
        self.shutdown_gate.mark_shutting_down().await;

        let downloading_ids: Vec<String> = self
            .store
            .read(|state| {
                state
                    .tasks()
                    .iter()
                    .filter(|t| t.status.is_downloading())
                    .map(|t| t.id.clone())
                    .collect()
            })
            .await;

        self.store
            .update_and_persist(|state| {
                state.prepare_for_exit();
                Ok(())
            })
            .await?;

        for id in &downloading_ids {
            self.store.reset_runtime_state(id).await;
        }

        Ok(())
    }

    pub async fn shutdown_status(&self) -> bool {
        self.shutdown_gate.status().await
    }
}

impl QueueStateReader for QueueManager {
    fn get_state_snapshot<'a>(&'a self) -> QueueRepositoryFuture<'a, QueueStateSnapshot> {
        Box::pin(async move { QueueManager::get_queue_state_snapshot(self).await })
    }

    fn live_work_status<'a>(&'a self) -> QueueRepositoryFuture<'a, bool> {
        Box::pin(async move { QueueManager::live_work_status(self).await })
    }

    fn shutdown_status<'a>(&'a self) -> QueueRepositoryFuture<'a, bool> {
        Box::pin(async move { QueueManager::shutdown_status(self).await })
    }

    fn pending_history_tasks<'a>(&'a self) -> QueueRepositoryFuture<'a, Vec<TaskSnapshot>> {
        Box::pin(async move { QueueManager::pending_history_tasks(self).await })
    }
}

impl QueueMutation for QueueManager {
    fn add_task<'a>(&'a self, task: Task) -> QueueRepositoryFuture<'a, AppResult<bool>> {
        Box::pin(async move { QueueManager::add_task(self, task).await })
    }

    fn remove_task<'a>(&'a self, id: &'a str) -> QueueRepositoryFuture<'a, AppResult<()>> {
        Box::pin(async move { QueueManager::remove_task(self, id).await })
    }

    fn retry_task<'a>(&'a self, id: &'a str) -> QueueRepositoryFuture<'a, AppResult<TaskSnapshot>> {
        Box::pin(async move { QueueManager::retry_task(self, id).await })
    }

    fn reorder_tasks<'a>(
        &'a self,
        task_ids: Vec<String>,
    ) -> QueueRepositoryFuture<'a, AppResult<()>> {
        Box::pin(async move { QueueManager::reorder_tasks(self, task_ids).await })
    }

    fn finish_run_if_idle<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<bool>> {
        Box::pin(async move { QueueManager::finish_run_if_idle(self).await })
    }

    fn set_run_status<'a>(
        &'a self,
        status: ApplicationQueueRunStatus,
    ) -> QueueRepositoryFuture<'a, AppResult<()>> {
        Box::pin(async move { QueueManager::set_run_status(self, status).await })
    }

    fn update_save_name<'a>(
        &'a self,
        id: &'a str,
        save_name: Option<String>,
    ) -> QueueRepositoryFuture<'a, AppResult<()>> {
        Box::pin(async move { QueueManager::update_save_name(self, id, save_name).await })
    }
}

impl QueueRunLifecycle for QueueManager {
    fn prepare_for_exit<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<()>> {
        Box::pin(async move { QueueManager::prepare_for_exit(self).await })
    }

    fn schedule_next<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>> {
        Box::pin(async move { QueueManager::schedule_next(self).await })
    }

    fn prepare_task_failure<'a>(
        &'a self,
        id: &'a str,
        error_message: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<ApplicationPrepareTaskFailureOutcome>> {
        Box::pin(async move { QueueManager::prepare_task_failure(self, id, error_message).await })
    }

    fn update_live_task_progress<'a>(
        &'a self,
        id: &'a str,
        progress: Option<f32>,
        speed: Option<String>,
        threads: Option<String>,
    ) -> QueueRepositoryFuture<'a, bool> {
        Box::pin(async move {
            QueueManager::update_live_task_progress(self, id, progress, speed, threads).await
        })
    }

    fn pause_after_failure_persistence_error<'a>(
        &'a self,
        id: &'a str,
        error_message: &'a str,
    ) -> QueueRepositoryFuture<'a, ()> {
        Box::pin(async move {
            QueueManager::pause_after_failure_persistence_error(self, id, error_message).await
        })
    }

    fn stage_task_completion<'a>(
        &'a self,
        id: &'a str,
        output_path: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>> {
        Box::pin(async move { QueueManager::stage_task_completion(self, id, output_path).await })
    }

    fn stage_terminal_history_task<'a>(
        &'a self,
        id: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>> {
        Box::pin(async move { QueueManager::stage_terminal_history_task(self, id).await })
    }

    fn clear_pending_history_task<'a>(
        &'a self,
        id: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<bool>> {
        Box::pin(async move { QueueManager::clear_pending_history_task(self, id).await })
    }
}

// QueueManager automatically implements the sum trait QueueRepository via the blanket impl
// once it implements the three narrow traits above. No hand-written forwarding of 18 methods needed.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::queue_repository_outcomes::{
        PrepareTaskFailureOutcome, QueueRunStatus, TaskFailureTransition,
    };

    use crate::application::queue_requests::AddTaskPayload;
    use crate::domain::task::TaskStatus;
    use chrono::Utc;

    fn task_from_payload(payload: AddTaskPayload) -> Task {
        Task::new_queued(
            uuid::Uuid::new_v4().to_string(),
            payload.url,
            payload.save_name,
            payload.headers,
            Utc::now(),
        )
    }

    async fn add_payload(
        manager: &QueueManager,
        payload: AddTaskPayload,
    ) -> AppResult<(Task, bool)> {
        let task = task_from_payload(payload);
        let outcome = manager.add_task(task.clone()).await?;
        Ok((task, outcome))
    }

    #[tokio::test]
    async fn add_task_keeps_paused_queue_paused() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        manager
            .set_run_status(QueueRunStatus::Paused)
            .await
            .expect("persist running state");
        let payload = AddTaskPayload {
            url: "https://example.com/paused.m3u8".to_string(),
            save_name: None,
            headers: None,
        };

        let (_, schedule_requested) = add_payload(&manager, payload).await.expect("add task");
        let state = manager.get_state().await;

        assert!(!state.is_running());
        assert!(
            !schedule_requested,
            "paused queue should add without requesting schedule"
        );
    }

    #[tokio::test]
    async fn add_task_requests_schedule_when_queue_is_running_and_idle() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("persist running state");
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };

        let (_, schedule_requested) = add_payload(&manager, payload).await.expect("add task");
        let state = manager.get_state().await;

        assert!(state.is_running());
        assert!(
            schedule_requested,
            "running idle queue should request schedule on add"
        );
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

        let result = add_payload(&manager, payload).await;

        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn add_task_rolls_back_memory_when_persistence_fails() {
        let path = std::env::temp_dir().join(format!("queue-state-dir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create blocking directory");
        let manager = QueueManager::new(path.clone());
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };

        let result = add_payload(&manager, payload).await;
        let state = manager.get_state().await;

        assert!(result.is_err());
        assert!(state.tasks().is_empty());

        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn set_run_status_rolls_back_memory_when_persistence_fails() {
        let path = std::env::temp_dir().join(format!("queue-state-dir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create blocking directory");
        let manager = QueueManager::new(path.clone());

        let result = manager.set_run_status(QueueRunStatus::Running).await;
        let state = manager.get_state().await;

        assert!(result.is_err());
        assert!(!state.is_running());

        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn prepare_for_exit_resets_downloading_state() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("persist running state");
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_payload(&manager, payload).await.expect("add task");
        manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("schedule first task");
        manager
            .update_live_task_progress(&task.id, Some(0.5), Some("1 MB/s".to_string()), None)
            .await;

        manager.prepare_for_exit().await.expect("prepare exit");

        let state = manager.get_state().await;
        let prepared = state
            .tasks()
            .iter()
            .find(|t| t.id == task.id)
            .expect("task exists");
        assert!(!state.is_running());
        assert!(state.current_task_id().is_none());
        assert_eq!(prepared.status, TaskStatus::Waiting);
    }

    #[tokio::test]
    async fn prepare_for_exit_resets_task_before_late_child_failure() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("persist running state");
        let payload = AddTaskPayload {
            url: "https://example.com/running.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_payload(&manager, payload).await.expect("add task");
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
            .tasks()
            .iter()
            .find(|t| t.id == task.id)
            .expect("task exists");
        assert!(!state.is_running());
        assert!(state.current_task_id().is_none());
        // After prepare_for_exit resets the task to Waiting with retry_count=0,
        // the domain schedules a retry rather than ignoring the failure.
        // Shutdown policy is now the application layer's responsibility.
        assert!(matches!(
            transition,
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        assert_eq!(prepared.status, TaskStatus::Waiting);
        assert_eq!(prepared.retry_count, 1);
    }

    #[tokio::test]
    async fn terminal_failure_preparation_releases_current_task_before_finalization() {
        let manager = QueueManager::new(
            std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4())),
        );
        let first = add_payload(
            &manager,
            AddTaskPayload {
                url: "https://example.com/first.m3u8".to_string(),
                save_name: Some("first".to_string()),
                headers: None,
            },
        )
        .await
        .expect("add first task")
        .0;
        let second = add_payload(
            &manager,
            AddTaskPayload {
                url: "https://example.com/second.m3u8".to_string(),
                save_name: Some("second".to_string()),
                headers: None,
            },
        )
        .await
        .expect("add second task")
        .0;

        manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("persist running state");
        manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("schedule first task");

        assert!(matches!(
            manager
                .prepare_task_failure(&first.id, "first failure")
                .await
                .expect("prepare first failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        manager
            .schedule_next()
            .await
            .expect("persist first retry")
            .expect("schedule first retry");
        assert!(matches!(
            manager
                .prepare_task_failure(&first.id, "second failure")
                .await
                .expect("prepare second failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        manager
            .schedule_next()
            .await
            .expect("persist second retry")
            .expect("schedule second retry");

        let transition = manager
            .prepare_task_failure(&first.id, "terminal failure")
            .await
            .expect("prepare terminal failure");

        assert!(matches!(
            transition,
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::Terminal)
        ));

        let state = manager.get_state().await;
        let failed = state
            .tasks()
            .iter()
            .find(|task| task.id == first.id)
            .expect("failed task remains until history finalization");
        assert!(state.current_task_id().is_none());
        assert_eq!(failed.status, TaskStatus::Failed);
        assert_eq!(failed.error_message.as_deref(), Some("terminal failure"));

        let scheduled = manager
            .schedule_next()
            .await
            .expect("schedule next after terminal preparation")
            .expect("next waiting task should be schedulable");
        assert_eq!(scheduled.id, second.id);
    }

    #[tokio::test]
    async fn reorder_waiting_tasks_persists_across_reload() {
        let path = std::env::temp_dir().join(format!("queue-state-{}.json", uuid::Uuid::new_v4()));
        let manager = QueueManager::new(path.clone());

        let (first, _) = add_payload(
            &manager,
            AddTaskPayload {
                url: "https://example.com/1.m3u8".to_string(),
                save_name: Some("first".to_string()),
                headers: None,
            },
        )
        .await
        .expect("add first task");
        let (second, _) = add_payload(
            &manager,
            AddTaskPayload {
                url: "https://example.com/2.m3u8".to_string(),
                save_name: Some("second".to_string()),
                headers: None,
            },
        )
        .await
        .expect("add second task");
        let (third, _) = add_payload(
            &manager,
            AddTaskPayload {
                url: "https://example.com/3.m3u8".to_string(),
                save_name: Some("third".to_string()),
                headers: None,
            },
        )
        .await
        .expect("add third task");

        manager
            .set_run_status(QueueRunStatus::Running)
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
        let ids: Vec<_> = state.tasks().iter().map(|task| task.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![first.id.as_str(), third.id.as_str(), second.id.as_str()]
        );

        let reloaded = QueueManager::new(path.clone());
        let reloaded_state = reloaded.get_state().await;
        let reloaded_ids: Vec<_> = reloaded_state
            .tasks()
            .iter()
            .map(|task| task.id.as_str())
            .collect();
        assert_eq!(reloaded_ids, ids);

        std::fs::remove_file(path).expect("cleanup queue state");
    }
}
