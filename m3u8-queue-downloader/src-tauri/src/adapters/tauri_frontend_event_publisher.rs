use crate::adapters::frontend_dto::TaskDto;
use crate::adapters::history_status_codec::history_status_slug;
use crate::application::task_snapshot::TaskSnapshot;
use crate::domain::history::HistoryStatus;
use crate::ports::event_publisher::FrontendEventPublisher;
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
pub(crate) struct TauriFrontendEventPublisher {
    app_handle: AppHandle,
}

impl TauriFrontendEventPublisher {
    pub(crate) fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl FrontendEventPublisher for TauriFrontendEventPublisher {
    fn task_error(&self, task_id: &str, message: &str) {
        let payload = serde_json::json!({
            "id": task_id,
            "message": message,
        });
        let _ = self.app_handle.emit("task-error", payload);
    }

    fn history_task_added(&self, status: HistoryStatus, task: &TaskSnapshot) {
        let payload = serde_json::json!({
            "status": history_status_slug(status),
            "task": TaskDto::from(task),
        });
        let _ = self.app_handle.emit("history-task-added", payload);
    }

    fn queue_state_changed(&self) {
        let _ = self.app_handle.emit("queue-state-changed", ());
    }

    fn shutdown_countdown_cancelled(&self) {
        let _ = self.app_handle.emit("shutdown-countdown-cancelled", ());
    }

    fn shutdown_countdown_started(&self, seconds: u64) {
        let payload = serde_json::json!({ "seconds": seconds });
        let _ = self.app_handle.emit("shutdown-countdown-started", payload);
    }

    fn task_progress(
        &self,
        task_id: &str,
        progress: Option<f32>,
        speed: Option<&str>,
        threads: Option<&str>,
    ) {
        let payload = serde_json::json!({
            "id": task_id,
            "progress": progress,
            "speed": speed,
            "threads": threads,
        });
        let _ = self.app_handle.emit("task-progress", payload);
    }

    fn terminal_committed_line(&self, task_id: &str, line: &str) {
        let payload = serde_json::json!({
            "id": task_id,
            "line": line,
        });
        let _ = self
            .app_handle
            .emit("task-terminal-committed-line", payload);
    }

    fn terminal_active_line(&self, task_id: &str, active_line: &str) {
        let payload = serde_json::json!({
            "id": task_id,
            "activeLine": active_line,
        });
        let _ = self.app_handle.emit("task-terminal-active-line", payload);
    }
}
