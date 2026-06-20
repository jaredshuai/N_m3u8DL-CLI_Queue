use crate::adapters::tauri_frontend_event_publisher::TauriFrontendEventPublisher;
use crate::application::task_process_events::{TaskLifecycleEvent, TaskOutputEvent};
use crate::composition::dependency_graph::DependencyGraph;
use crate::composition::runtime_facade::RuntimeFacade;
use tauri::AppHandle;
use tokio::sync::mpsc;

pub(crate) fn spawn_task_lifecycle_worker(
    app_handle: AppHandle,
    state: DependencyGraph,
    mut receiver: mpsc::UnboundedReceiver<TaskLifecycleEvent>,
) {
    tauri::async_runtime::spawn(async move {
        let events = TauriFrontendEventPublisher::new(app_handle.clone());
        let runtime = RuntimeFacade::new(state);
        while let Some(event) = receiver.recv().await {
            runtime.handle_task_lifecycle_event(&events, event).await;
        }
    });
}

pub(crate) fn spawn_task_output_worker(
    app_handle: AppHandle,
    state: DependencyGraph,
    mut receiver: mpsc::UnboundedReceiver<TaskOutputEvent>,
) {
    tauri::async_runtime::spawn(async move {
        let events = TauriFrontendEventPublisher::new(app_handle);
        let runtime = RuntimeFacade::new(state);
        while let Some(event) = receiver.recv().await {
            runtime.handle_task_output_event(&events, event).await;
        }
    });
}
