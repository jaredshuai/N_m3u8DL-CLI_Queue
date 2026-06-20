use crate::application::app_error::AppResult;
use crate::application::queue_repository_outcomes::{PrepareTaskFailureOutcome, QueueRunStatus};
use crate::application::queue_state_snapshot::QueueStateSnapshot;
use crate::application::task_snapshot::TaskSnapshot;
use crate::domain::task::Task;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub(crate) type QueueRepositoryFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Read-only snapshot and status queries (≤7 methods).
pub(crate) trait QueueStateReader: Send + Sync {
    fn get_state_snapshot<'a>(&'a self) -> QueueRepositoryFuture<'a, QueueStateSnapshot>;
    fn live_work_status<'a>(&'a self) -> QueueRepositoryFuture<'a, bool>;
    fn shutdown_status<'a>(&'a self) -> QueueRepositoryFuture<'a, bool>;
    fn pending_history_tasks<'a>(&'a self) -> QueueRepositoryFuture<'a, Vec<TaskSnapshot>>;
}

/// Queue structural mutations (add/remove/retry/reorder/run-status) (≤7 methods).
pub(crate) trait QueueMutation: Send + Sync {
    fn add_task<'a>(&'a self, task: Task) -> QueueRepositoryFuture<'a, AppResult<bool>>;
    fn remove_task<'a>(&'a self, id: &'a str) -> QueueRepositoryFuture<'a, AppResult<()>>;
    fn retry_task<'a>(&'a self, id: &'a str) -> QueueRepositoryFuture<'a, AppResult<TaskSnapshot>>;
    fn reorder_tasks<'a>(
        &'a self,
        task_ids: Vec<String>,
    ) -> QueueRepositoryFuture<'a, AppResult<()>>;
    fn finish_run_if_idle<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<bool>>;
    fn set_run_status<'a>(
        &'a self,
        status: QueueRunStatus,
    ) -> QueueRepositoryFuture<'a, AppResult<()>>;
    fn update_save_name<'a>(
        &'a self,
        id: &'a str,
        save_name: Option<String>,
    ) -> QueueRepositoryFuture<'a, AppResult<()>>;
}

/// Run lifecycle, scheduling and per-task run-time staging (≤8 methods).
pub(crate) trait QueueRunLifecycle: Send + Sync {
    fn prepare_for_exit<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<()>>;
    fn schedule_next<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>>;
    fn prepare_task_failure<'a>(
        &'a self,
        id: &'a str,
        error_message: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<PrepareTaskFailureOutcome>>;
    fn update_live_task_progress<'a>(
        &'a self,
        id: &'a str,
        progress: Option<f32>,
        speed: Option<String>,
        threads: Option<String>,
    ) -> QueueRepositoryFuture<'a, bool>;
    fn pause_after_failure_persistence_error<'a>(
        &'a self,
        id: &'a str,
        error_message: &'a str,
    ) -> QueueRepositoryFuture<'a, ()>;
    fn stage_task_completion<'a>(
        &'a self,
        id: &'a str,
        output_path: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>>;
    fn stage_terminal_history_task<'a>(
        &'a self,
        id: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>>;
    fn clear_pending_history_task<'a>(
        &'a self,
        id: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<bool>>;
}

/// Sum trait for backward compatibility.
/// Code using `&dyn QueueRepository` or `Arc<dyn QueueRepository>` continues to work.
pub(crate) trait QueueRepository:
    QueueStateReader + QueueMutation + QueueRunLifecycle + Send + Sync
{
}

impl<T> QueueRepository for T where
    T: QueueStateReader + QueueMutation + QueueRunLifecycle + Send + Sync
{
}

// ---- Arc<T> forwarding for narrow traits (explicit, to guarantee late-bound lifetimes match trait defs) ----
// No 18-method hand-written block for the old god trait. Macro removed to avoid early-bound lifetime issues.

impl<T> QueueStateReader for Arc<T>
where
    T: QueueStateReader + ?Sized,
{
    fn get_state_snapshot<'a>(&'a self) -> QueueRepositoryFuture<'a, QueueStateSnapshot> {
        self.as_ref().get_state_snapshot()
    }
    fn live_work_status<'a>(&'a self) -> QueueRepositoryFuture<'a, bool> {
        self.as_ref().live_work_status()
    }
    fn shutdown_status<'a>(&'a self) -> QueueRepositoryFuture<'a, bool> {
        self.as_ref().shutdown_status()
    }
    fn pending_history_tasks<'a>(&'a self) -> QueueRepositoryFuture<'a, Vec<TaskSnapshot>> {
        self.as_ref().pending_history_tasks()
    }
}

impl<T> QueueMutation for Arc<T>
where
    T: QueueMutation + ?Sized,
{
    fn add_task<'a>(&'a self, task: Task) -> QueueRepositoryFuture<'a, AppResult<bool>> {
        self.as_ref().add_task(task)
    }
    fn remove_task<'a>(&'a self, id: &'a str) -> QueueRepositoryFuture<'a, AppResult<()>> {
        self.as_ref().remove_task(id)
    }
    fn retry_task<'a>(&'a self, id: &'a str) -> QueueRepositoryFuture<'a, AppResult<TaskSnapshot>> {
        self.as_ref().retry_task(id)
    }
    fn reorder_tasks<'a>(
        &'a self,
        task_ids: Vec<String>,
    ) -> QueueRepositoryFuture<'a, AppResult<()>> {
        self.as_ref().reorder_tasks(task_ids)
    }
    fn finish_run_if_idle<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<bool>> {
        self.as_ref().finish_run_if_idle()
    }
    fn set_run_status<'a>(
        &'a self,
        status: QueueRunStatus,
    ) -> QueueRepositoryFuture<'a, AppResult<()>> {
        self.as_ref().set_run_status(status)
    }
    fn update_save_name<'a>(
        &'a self,
        id: &'a str,
        save_name: Option<String>,
    ) -> QueueRepositoryFuture<'a, AppResult<()>> {
        self.as_ref().update_save_name(id, save_name)
    }
}

impl<T> QueueRunLifecycle for Arc<T>
where
    T: QueueRunLifecycle + ?Sized,
{
    fn prepare_for_exit<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<()>> {
        self.as_ref().prepare_for_exit()
    }
    fn schedule_next<'a>(&'a self) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>> {
        self.as_ref().schedule_next()
    }
    fn prepare_task_failure<'a>(
        &'a self,
        id: &'a str,
        error_message: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<PrepareTaskFailureOutcome>> {
        self.as_ref().prepare_task_failure(id, error_message)
    }
    fn update_live_task_progress<'a>(
        &'a self,
        id: &'a str,
        progress: Option<f32>,
        speed: Option<String>,
        threads: Option<String>,
    ) -> QueueRepositoryFuture<'a, bool> {
        self.as_ref()
            .update_live_task_progress(id, progress, speed, threads)
    }
    fn pause_after_failure_persistence_error<'a>(
        &'a self,
        id: &'a str,
        error_message: &'a str,
    ) -> QueueRepositoryFuture<'a, ()> {
        self.as_ref()
            .pause_after_failure_persistence_error(id, error_message)
    }
    fn stage_task_completion<'a>(
        &'a self,
        id: &'a str,
        output_path: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>> {
        self.as_ref().stage_task_completion(id, output_path)
    }
    fn stage_terminal_history_task<'a>(
        &'a self,
        id: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<Option<TaskSnapshot>>> {
        self.as_ref().stage_terminal_history_task(id)
    }
    fn clear_pending_history_task<'a>(
        &'a self,
        id: &'a str,
    ) -> QueueRepositoryFuture<'a, AppResult<bool>> {
        self.as_ref().clear_pending_history_task(id)
    }
}

// No more 18-method hand-written `impl<T> QueueRepository for Arc<T>` block.
// Arc<T> forwarding for the narrow traits is provided by the three explicit impl blocks above.
