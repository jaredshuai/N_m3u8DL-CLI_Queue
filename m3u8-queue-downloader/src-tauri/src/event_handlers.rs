use crate::app_state::AppState;
use crate::runtime;
use std::future::Future;
use std::sync::Arc;
use tauri::{AppHandle, Listener};

pub(crate) fn register_event_handlers(
    app: &mut tauri::App,
    app_handle: AppHandle,
    state: AppState,
) {
    register_json_listener(
        app,
        "task-completed",
        state.clone(),
        app_handle.clone(),
        |state, app_handle, data| async move {
            let task_id = data["id"].as_str().unwrap_or("").to_string();
            let output_path = data["outputPath"].as_str().unwrap_or("").to_string();
            runtime::handle_task_completed(state, app_handle, task_id, output_path).await;
        },
    );

    register_json_listener(
        app,
        "task-failed",
        state.clone(),
        app_handle.clone(),
        |state, app_handle, data| async move {
            let task_id = data["id"].as_str().unwrap_or("").to_string();
            let error_message = data["errorMessage"]
                .as_str()
                .unwrap_or("Unknown error")
                .to_string();
            runtime::handle_task_failed(state, app_handle, task_id, error_message).await;
        },
    );

    register_json_listener(
        app,
        "task-progress",
        state.clone(),
        app_handle.clone(),
        |state, _app_handle, data| async move {
            let task_id = data["id"].as_str().unwrap_or("").to_string();
            let progress = data["progress"]
                .as_f64()
                .map(|value| value as f32)
                .filter(|value| *value >= 0.0);
            let speed = data["speed"]
                .as_str()
                .map(str::to_string)
                .filter(|value| !value.is_empty());
            let threads = data["threads"]
                .as_str()
                .map(str::to_string)
                .filter(|value| !value.is_empty());
            state
                .queue_manager
                .update_task_progress(&task_id, progress, speed, threads)
                .await;
        },
    );

    register_json_listener(
        app,
        "task-log",
        state.clone(),
        app_handle.clone(),
        |state, _app_handle, data| async move {
            let task_id = data["id"].as_str().unwrap_or("").to_string();
            let line = data["line"].as_str().unwrap_or("").to_string();
            if let Err(err) = state.cli_output_store.append_line(&task_id, &line) {
                eprintln!("Failed to persist CLI live output: {}", err);
            }
        },
    );

    register_json_listener(
        app,
        "task-terminal-active-line",
        state,
        app_handle,
        |state, _app_handle, data| async move {
            let task_id = data["id"].as_str().unwrap_or("").to_string();
            let active_line = data["activeLine"].as_str().unwrap_or("").to_string();
            if active_line.is_empty() {
                state.cli_output_store.clear_active_line(&task_id);
            } else {
                state
                    .cli_output_store
                    .set_active_line(&task_id, active_line);
            }
        },
    );
}

fn register_json_listener<F, Fut>(
    app: &mut tauri::App,
    event_name: &'static str,
    state: AppState,
    app_handle: AppHandle,
    handler: F,
) where
    F: Fn(AppState, AppHandle, serde_json::Value) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let handler = Arc::new(handler);
    app.listen(event_name, move |event: tauri::Event| {
        let state = state.clone();
        let app_handle = app_handle.clone();
        let payload = event.payload().to_string();
        let handler = Arc::clone(&handler);
        tauri::async_runtime::spawn(async move {
            match serde_json::from_str::<serde_json::Value>(&payload) {
                Ok(data) => handler(state, app_handle, data).await,
                Err(err) => eprintln!("Failed to parse {event_name} payload: {}", err),
            }
        });
    });
}
