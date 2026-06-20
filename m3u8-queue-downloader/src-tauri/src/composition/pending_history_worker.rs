use crate::adapters::tauri_frontend_event_publisher::TauriFrontendEventPublisher;
use crate::composition::dependency_graph::DependencyGraph;
use crate::composition::diagnostics_facade::DiagnosticsFacade;
use crate::composition::pending_history_facade::PendingHistoryFacade;
use crate::ports::event_publisher::FrontendEventPublisher;
use tauri::AppHandle;

pub(crate) fn spawn_pending_history_flush(state: DependencyGraph, app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let events = TauriFrontendEventPublisher::new(app_handle);
        let pending_history = PendingHistoryFacade::new(state.clone());
        match pending_history.flush_pending_history_tasks().await {
            Ok(tasks) => {
                for task in &tasks {
                    events.history_task_added(task.status, &task.task);
                }
                if !tasks.is_empty() {
                    events.queue_state_changed();
                }
            }
            Err(err) => {
                let message = format!("恢复未写入历史记录时出错：{}", err);
                DiagnosticsFacade::new(state).warn(&message);
                events.task_error("pending-history", &message);
            }
        }
    });
}
