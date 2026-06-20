use crate::application::task_snapshot::TaskSnapshot;
use crate::domain::history::HistoryStatus;

pub(crate) trait FrontendEventPublisher: Send + Sync {
    fn task_error(&self, task_id: &str, message: &str);
    fn history_task_added(&self, status: HistoryStatus, task: &TaskSnapshot);
    fn queue_state_changed(&self);
    fn shutdown_countdown_cancelled(&self);
    fn shutdown_countdown_started(&self, seconds: u64);
    fn task_progress(
        &self,
        task_id: &str,
        progress: Option<f32>,
        speed: Option<&str>,
        threads: Option<&str>,
    );
    fn terminal_committed_line(&self, task_id: &str, line: &str);
    fn terminal_active_line(&self, task_id: &str, active_line: &str);
}
