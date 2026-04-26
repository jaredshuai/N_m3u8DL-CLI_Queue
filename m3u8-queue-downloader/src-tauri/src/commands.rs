use crate::app_error::{AppError, AppResult};
use crate::app_state::AppState;
use crate::download_dir::resolve_download_dir;
use crate::models::{
    AddTaskPayload, AppSettings, CliOutputPage, CliTerminalState, HistoryPage, HistoryStatus,
    QueueState, Task,
};
use crate::runtime::{self, CloseRequestSource};
use tauri::{Emitter, Manager, State};

fn command_result<T>(result: AppResult<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

async fn spawn_blocking_result<T, F>(context: &str, work: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| AppError::message(format!("{context}: {err}")))?
}

#[tauri::command]
pub async fn get_queue_state(state: State<'_, AppState>) -> Result<QueueState, String> {
    Ok(state.queue_manager.get_state().await)
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings_store.get())
}

#[tauri::command]
pub fn update_app_settings(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    command_result(update_app_settings_impl(&state, &app_handle, settings))
}

fn update_app_settings_impl(
    state: &AppState,
    app_handle: &tauri::AppHandle,
    settings: AppSettings,
) -> AppResult<AppSettings> {
    let previous = state.settings_store.get();
    let updated = state.settings_store.update(settings)?;

    if !previous.auto_action_on_complete && updated.auto_action_on_complete {
        state.shutdown_manager.clear_cancellation_after_reenable();
    }

    if previous.auto_action_on_complete && !updated.auto_action_on_complete {
        let _ = state.shutdown_manager.cancel_countdown();
        let _ = app_handle.emit("shutdown-countdown-cancelled", ());
    }

    Ok(updated)
}

#[tauri::command]
pub async fn get_history_page(
    state: State<'_, AppState>,
    status: HistoryStatus,
    offset: usize,
    limit: usize,
) -> Result<HistoryPage, String> {
    let history_store = state.history_store.clone();
    command_result(
        spawn_blocking_result("history page task failed to join", move || {
            history_store.get_page(status, offset, limit)
        })
        .await,
    )
}

#[tauri::command]
pub async fn get_cli_output_tail(
    state: State<'_, AppState>,
    task_id: String,
    limit: usize,
) -> Result<CliOutputPage, String> {
    let cli_output_store = state.cli_output_store.clone();
    command_result(
        spawn_blocking_result("cli output tail task failed to join", move || {
            cli_output_store.tail(&task_id, limit)
        })
        .await,
    )
}

#[tauri::command]
pub async fn get_cli_output_page(
    state: State<'_, AppState>,
    task_id: String,
    offset: usize,
    limit: usize,
) -> Result<CliOutputPage, String> {
    let cli_output_store = state.cli_output_store.clone();
    command_result(
        spawn_blocking_result("cli output page task failed to join", move || {
            cli_output_store.page(&task_id, offset, limit)
        })
        .await,
    )
}

#[tauri::command]
pub async fn get_cli_terminal_state(
    state: State<'_, AppState>,
    task_id: String,
    limit: usize,
) -> Result<CliTerminalState, String> {
    let cli_output_store = state.cli_output_store.clone();
    command_result(
        spawn_blocking_result("cli terminal state task failed to join", move || {
            let page = cli_output_store.tail(&task_id, limit)?;
            let active_line = cli_output_store
                .get_active_line(&task_id)
                .unwrap_or_default();
            Ok(CliTerminalState {
                committed_lines: page.lines,
                active_line,
                offset: page.offset,
                total: page.total,
                has_more_before: page.has_more_before,
            })
        })
        .await,
    )
}

#[tauri::command]
pub async fn add_task(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    url: String,
    save_name: Option<String>,
    headers: Option<String>,
) -> Result<Task, String> {
    let payload = AddTaskPayload {
        url,
        save_name,
        headers,
    };
    let (task, should_schedule) = command_result(state.queue_manager.add_task(payload).await)?;
    if should_schedule {
        let download_dir = resolve_download_dir(&state.settings_store.get());
        command_result(
            runtime::try_schedule_next(
                &state.queue_manager,
                &state.history_store,
                &state.task_runner,
                &download_dir,
                &app_handle,
            )
            .await,
        )?;
    }
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(task)
}

#[tauri::command]
pub async fn remove_task(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    task_id: String,
) -> Result<(), String> {
    command_result(state.queue_manager.remove_task(&task_id).await)?;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn remove_history_task(
    state: State<'_, AppState>,
    status: HistoryStatus,
    task_id: String,
) -> Result<(), String> {
    command_result(remove_history_task_impl(&state, status, &task_id))
}

fn remove_history_task_impl(
    state: &AppState,
    status: HistoryStatus,
    task_id: &str,
) -> AppResult<()> {
    if state.history_store.remove_task(status, task_id)? {
        return Ok(());
    }

    Err(AppError::message(format!(
        "History task {task_id} not found"
    )))
}

#[tauri::command]
pub async fn retry_task(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    task_id: String,
) -> Result<Task, String> {
    command_result(retry_task_impl(&state, &app_handle, &task_id).await)
}

async fn retry_task_impl(
    state: &AppState,
    app_handle: &tauri::AppHandle,
    task_id: &str,
) -> AppResult<Task> {
    let task = match state.queue_manager.retry_task(task_id).await {
        Ok(task) => task,
        Err(AppError::TaskNotFound { .. }) => {
            let history_task = state
                .history_store
                .find_task(HistoryStatus::Failed, task_id)?
                .ok_or_else(|| AppError::TaskNotFound {
                    id: task_id.to_string(),
                })?;
            let (task, should_schedule) = state
                .queue_manager
                .add_history_retry_task(&history_task)
                .await?;
            if should_schedule {
                let download_dir = resolve_download_dir(&state.settings_store.get());
                runtime::try_schedule_next(
                    &state.queue_manager,
                    &state.history_store,
                    &state.task_runner,
                    &download_dir,
                    app_handle,
                )
                .await?;
                let _ = app_handle.emit("queue-state-changed", ());
                return Ok(task);
            }
            task
        }
        Err(err) => return Err(err),
    };

    let download_dir = resolve_download_dir(&state.settings_store.get());
    runtime::try_schedule_next(
        &state.queue_manager,
        &state.history_store,
        &state.task_runner,
        &download_dir,
        app_handle,
    )
    .await?;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(task)
}

#[tauri::command]
pub async fn reorder_tasks(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    task_ids: Vec<String>,
) -> Result<(), String> {
    command_result(state.queue_manager.reorder_tasks(task_ids).await)?;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn start_queue(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    command_result(runtime::start_queue_internal(&state, &app_handle).await)
}

#[tauri::command]
pub async fn pause_queue(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    command_result(runtime::pause_queue_internal(&state.queue_manager).await)?;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(())
}

#[tauri::command]
pub fn minimize_main_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    command_result(minimize_main_window_impl(&app_handle))
}

fn minimize_main_window_impl(app_handle: &tauri::AppHandle) -> AppResult<()> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .minimize()
            .map_err(|e| AppError::message(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_main_window_maximize(app_handle: tauri::AppHandle) -> Result<(), String> {
    command_result(toggle_main_window_maximize_impl(&app_handle))
}

fn toggle_main_window_maximize_impl(app_handle: &tauri::AppHandle) -> AppResult<()> {
    if let Some(window) = app_handle.get_webview_window("main") {
        if window
            .is_maximized()
            .map_err(|e| AppError::message(e.to_string()))?
        {
            window
                .unmaximize()
                .map_err(|e| AppError::message(e.to_string()))?;
        } else {
            window
                .maximize()
                .map_err(|e| AppError::message(e.to_string()))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn request_main_window_close(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    command_result(
        runtime::request_close_internal(&state, app_handle, CloseRequestSource::WindowButton).await,
    )
}

#[tauri::command]
pub fn open_download_dir(state: State<'_, AppState>) -> Result<(), String> {
    command_result(runtime::open_download_dir_internal(&state))
}

#[tauri::command]
pub fn cancel_auto_shutdown(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    command_result(runtime::cancel_auto_shutdown_internal(&state, &app_handle))
}
