use crate::app_state::AppState;
use crate::runtime;
use crate::task_runner::{TaskLifecycleEvent, TaskOutputEvent};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

pub(crate) fn spawn_task_lifecycle_worker(
    app_handle: AppHandle,
    state: AppState,
    mut receiver: mpsc::UnboundedReceiver<TaskLifecycleEvent>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = receiver.recv().await {
            match event {
                TaskLifecycleEvent::Completed { id, output_path } => {
                    runtime::handle_task_completed(
                        state.clone(),
                        app_handle.clone(),
                        id,
                        output_path,
                    )
                    .await;
                }
                TaskLifecycleEvent::Failed { id, error_message } => {
                    runtime::handle_task_failed(
                        state.clone(),
                        app_handle.clone(),
                        id,
                        error_message,
                    )
                    .await;
                }
            }
        }
    });
}

pub(crate) fn spawn_task_output_worker(
    app_handle: AppHandle,
    state: AppState,
    mut receiver: mpsc::UnboundedReceiver<TaskOutputEvent>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = receiver.recv().await {
            match event {
                TaskOutputEvent::Progress {
                    id,
                    progress,
                    speed,
                    threads,
                } => {
                    state
                        .queue_manager
                        .update_task_progress(&id, progress, speed.clone(), threads.clone())
                        .await;
                    let payload = serde_json::json!({
                        "id": id,
                        "progress": progress,
                        "speed": speed,
                        "threads": threads,
                    });
                    let _ = app_handle.emit("task-progress", payload);
                }
                TaskOutputEvent::LogLine { id, line } => {
                    if let Err(err) = state.cli_output_store.append_line(&id, &line) {
                        eprintln!("Failed to persist CLI live output: {}", err);
                    }
                }
                TaskOutputEvent::TerminalCommittedLine { id, line } => {
                    let payload = serde_json::json!({
                        "id": id,
                        "line": line,
                    });
                    let _ = app_handle.emit("task-terminal-committed-line", payload);
                }
                TaskOutputEvent::TerminalActiveLine { id, active_line } => {
                    if active_line.is_empty() {
                        state.cli_output_store.clear_active_line(&id);
                    } else {
                        state
                            .cli_output_store
                            .set_active_line(&id, active_line.clone());
                    }
                    let payload = serde_json::json!({
                        "id": id,
                        "activeLine": active_line,
                    });
                    let _ = app_handle.emit("task-terminal-active-line", payload);
                }
            }
        }
    });
}
