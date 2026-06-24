use crate::application::app_error::AppResult;
use crate::application::process_runner_outcomes::ProcessRunnerShutdownStatus;
use crate::application::task_process_start_request::TaskProcessStartRequest;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub(crate) type ProcessRunnerFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) trait TaskProcessRunner: Send + Sync {
    fn start_task<'a>(
        &'a self,
        request: TaskProcessStartRequest,
    ) -> ProcessRunnerFuture<'a, AppResult<()>>;

    fn shutdown_status<'a>(&'a self) -> ProcessRunnerFuture<'a, ProcessRunnerShutdownStatus>;
}

pub(crate) trait TaskProcessSupervisor: Send + Sync {
    fn begin_shutdown<'a>(&'a self) -> ProcessRunnerFuture<'a, ()>;
    fn terminate_all_running_processes<'a>(&'a self) -> ProcessRunnerFuture<'a, AppResult<()>>;
    /// Kill a single running child process by task id and emit a `Cancelled`
    /// lifecycle event. Safe to call when the process has already exited
    /// (returns Ok in that case). See ADR-0009.
    fn terminate_task<'a>(
        &'a self,
        task_id: &'a str,
    ) -> ProcessRunnerFuture<'a, AppResult<()>>;
}

pub(crate) trait TaskProcessRunnerFactory: Send + Sync {
    fn create_process_runner(&self) -> Arc<dyn TaskProcessRunner>;
}
