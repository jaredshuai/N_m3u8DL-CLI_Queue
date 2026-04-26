use crate::app_error::{AppError, AppResult};
use crate::app_state::AppState;
use crate::download_dir::resolve_download_dir;
use crate::history_store::HistoryStore;
use crate::models::{AppSettings, CloseButtonBehavior, HistoryStatus, Task};
use crate::queue_manager::{QueueManager, TaskFailureTransition};
use crate::shutdown::ShutdownManager;
use crate::task_runner::TaskRunner;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseRequestSource {
    WindowButton,
    TrayQuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseAction {
    HideToTray,
    ExitApplication,
}

pub(crate) async fn start_queue_internal(
    state: &AppState,
    app_handle: &AppHandle,
) -> AppResult<()> {
    let download_dir = resolve_download_dir(&state.settings_store.get());
    if state.shutdown_manager.reset_for_new_run()? {
        let _ = app_handle.emit("shutdown-countdown-cancelled", ());
    }
    if !state.queue_manager.has_live_work().await {
        state.queue_manager.set_running(false).await?;
        let _ = app_handle.emit("queue-state-changed", ());
        return Ok(());
    }

    state.queue_manager.set_running(true).await?;
    try_schedule_next(
        &state.queue_manager,
        &state.history_store,
        &state.task_runner,
        &download_dir,
        app_handle,
    )
    .await?;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(())
}

pub(crate) async fn pause_queue_internal(queue_manager: &Arc<QueueManager>) -> AppResult<()> {
    queue_manager.set_running(false).await
}

pub(crate) async fn request_close_internal(
    state: &AppState,
    app_handle: AppHandle,
    source: CloseRequestSource,
) -> AppResult<()> {
    match resolve_close_action(state.settings_store.get().close_button_behavior, source) {
        CloseAction::HideToTray => hide_main_window(&app_handle),
        CloseAction::ExitApplication => {
            exit_application(
                Arc::clone(&state.queue_manager),
                Arc::clone(&state.task_runner),
                app_handle,
            )
            .await
        }
    }
}

pub(crate) fn open_download_dir_internal(state: &AppState) -> AppResult<()> {
    open_download_dir_for_path(resolve_download_dir(&state.settings_store.get()))
}

pub(crate) fn cancel_auto_shutdown_internal(
    state: &AppState,
    app_handle: &AppHandle,
) -> AppResult<()> {
    state.shutdown_manager.cancel_countdown()?;
    let _ = app_handle.emit("shutdown-countdown-cancelled", ());
    Ok(())
}

pub(crate) async fn try_schedule_next(
    queue_manager: &Arc<QueueManager>,
    history_store: &Arc<HistoryStore>,
    task_runner: &Arc<TaskRunner>,
    download_dir: &PathBuf,
    app_handle: &AppHandle,
) -> AppResult<()> {
    loop {
        let task = match queue_manager.schedule_next().await {
            Ok(Some(task)) => task,
            Ok(None) => return Ok(()),
            Err(err) => return Err(err),
        };
        let task_id = task.id.clone();
        match task_runner
            .start_task(task, download_dir.clone(), app_handle.clone())
            .await
        {
            Ok(()) => return Ok(()),
            Err(err) => {
                let error_message = err.to_string();
                eprintln!("Failed to start task {}: {}", task_id, error_message);
                match handle_start_failure(queue_manager, history_store, &task_id, &error_message)
                    .await
                {
                    Ok(Some(task)) => {
                        let payload = serde_json::json!({
                            "status": HistoryStatus::Failed,
                            "task": task,
                        });
                        let _ = app_handle.emit("history-task-added", payload);
                    }
                    Ok(None) => {}
                    Err(persist_err) => {
                        let message =
                            format!("任务启动失败，但写入失败历史时出错：{}", persist_err);
                        eprintln!("{message}");
                        emit_task_error(app_handle, &task_id, message);
                    }
                }
            }
        }
    }
}

pub(crate) async fn handle_start_failure(
    queue_manager: &Arc<QueueManager>,
    history_store: &Arc<HistoryStore>,
    task_id: &str,
    error_message: &str,
) -> AppResult<Option<Task>> {
    match queue_manager
        .prepare_task_failure(task_id, error_message)
        .await?
    {
        Some(TaskFailureTransition::RetryScheduled) => Ok(None),
        Some(TaskFailureTransition::Terminal(task)) => {
            let task =
                record_terminal_failure_task(queue_manager, history_store, task_id, task).await?;
            Ok(Some(task))
        }
        None => Ok(None),
    }
}

pub(crate) async fn record_completed_task(
    queue_manager: &Arc<QueueManager>,
    history_store: &Arc<HistoryStore>,
    task_id: &str,
    output_path: &str,
) -> AppResult<Option<Task>> {
    let Some(task) = queue_manager
        .snapshot_task_completion(task_id, output_path)
        .await
    else {
        return Ok(None);
    };

    let append_result = history_store.append(&task);
    queue_manager.finalize_task_completion(task_id).await?;
    append_result.map(|_| Some(task))
}

pub(crate) async fn record_terminal_failure_task(
    queue_manager: &Arc<QueueManager>,
    history_store: &Arc<HistoryStore>,
    task_id: &str,
    task: Task,
) -> AppResult<Task> {
    let append_result = history_store.append(&task);
    queue_manager.finalize_terminal_failure(task_id).await?;
    append_result.map(|_| task)
}

pub(crate) async fn handle_task_completed(
    state: AppState,
    app_handle: AppHandle,
    task_id: String,
    output_path: String,
) {
    state.cli_output_store.clear_active_line(&task_id);
    if state.queue_manager.is_shutting_down().await {
        let _ = app_handle.emit("queue-state-changed", ());
        return;
    }

    match record_completed_task(
        &state.queue_manager,
        &state.history_store,
        &task_id,
        &output_path,
    )
    .await
    {
        Ok(Some(task)) => {
            let payload = serde_json::json!({
                "status": HistoryStatus::Completed,
                "task": task,
            });
            let _ = app_handle.emit("history-task-added", payload);
        }
        Ok(None) => {}
        Err(err) => {
            let message = format!("任务已完成，但写入完成历史时出错：{}", err);
            eprintln!("{message}");
            emit_task_error(&app_handle, &task_id, message);
        }
    }
    let _ = app_handle.emit("queue-state-changed", ());
    let download_dir = resolve_download_dir(&state.settings_store.get());
    if let Err(err) = try_schedule_next(
        &state.queue_manager,
        &state.history_store,
        &state.task_runner,
        &download_dir,
        &app_handle,
    )
    .await
    {
        eprintln!("Failed to schedule next task after completion: {}", err);
    }
    match state.queue_manager.finish_run_if_idle().await {
        Ok(true) => {
            let _ = app_handle.emit("queue-state-changed", ());
            if let Ok(Some(seconds)) = maybe_start_shutdown_countdown(
                &state.queue_manager,
                &state.shutdown_manager,
                &state.settings_store.get(),
            )
            .await
            {
                let payload = serde_json::json!({ "seconds": seconds });
                let _ = app_handle.emit("shutdown-countdown-started", payload);
            }
        }
        Ok(false) => {}
        Err(err) => eprintln!("Failed to finish idle run after completion: {}", err),
    }
}

pub(crate) async fn handle_task_failed(
    state: AppState,
    app_handle: AppHandle,
    task_id: String,
    error_message: String,
) {
    state.cli_output_store.clear_active_line(&task_id);
    if state.queue_manager.is_shutting_down().await {
        let _ = app_handle.emit("queue-state-changed", ());
        return;
    }

    match state
        .queue_manager
        .prepare_task_failure(&task_id, &error_message)
        .await
    {
        Ok(Some(TaskFailureTransition::RetryScheduled)) | Ok(None) => {}
        Ok(Some(TaskFailureTransition::Terminal(task))) => {
            state.shutdown_manager.mark_run_failure();
            match record_terminal_failure_task(
                &state.queue_manager,
                &state.history_store,
                &task_id,
                task,
            )
            .await
            {
                Ok(task) => {
                    let payload = serde_json::json!({
                        "status": HistoryStatus::Failed,
                        "task": task,
                    });
                    let _ = app_handle.emit("history-task-added", payload);
                }
                Err(err) => {
                    let message = format!("任务已失败，但写入失败历史时出错：{}", err);
                    eprintln!("{message}");
                    emit_task_error(&app_handle, &task_id, message);
                }
            }
        }
        Err(err) => eprintln!("Failed to prepare task failure: {}", err),
    }
    let _ = app_handle.emit("queue-state-changed", ());
    let download_dir = resolve_download_dir(&state.settings_store.get());
    if let Err(err) = try_schedule_next(
        &state.queue_manager,
        &state.history_store,
        &state.task_runner,
        &download_dir,
        &app_handle,
    )
    .await
    {
        eprintln!("Failed to schedule next task after failure: {}", err);
    }
    match state.queue_manager.finish_run_if_idle().await {
        Ok(true) => {
            let _ = app_handle.emit("queue-state-changed", ());
            if let Ok(Some(seconds)) = maybe_start_shutdown_countdown(
                &state.queue_manager,
                &state.shutdown_manager,
                &state.settings_store.get(),
            )
            .await
            {
                let payload = serde_json::json!({ "seconds": seconds });
                let _ = app_handle.emit("shutdown-countdown-started", payload);
            }
        }
        Ok(false) => {}
        Err(err) => eprintln!("Failed to finish idle run after failure: {}", err),
    }
}

pub(crate) async fn maybe_start_shutdown_countdown(
    queue_manager: &Arc<QueueManager>,
    shutdown_manager: &Arc<ShutdownManager>,
    settings: &AppSettings,
) -> AppResult<Option<u64>> {
    if !settings.auto_action_on_complete {
        return Ok(None);
    }

    if queue_manager.has_live_work().await {
        return Ok(None);
    }

    if !shutdown_manager.should_start_countdown() {
        return Ok(None);
    }

    let seconds = shutdown_manager.start_countdown()?;
    Ok(Some(seconds))
}

pub(crate) fn show_main_window(app_handle: &AppHandle) -> AppResult<()> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .show()
            .map_err(|e| AppError::message(e.to_string()))?;
        window
            .unminimize()
            .map_err(|e| AppError::message(e.to_string()))?;
        window
            .set_focus()
            .map_err(|e| AppError::message(e.to_string()))?;
    }
    Ok(())
}

pub(crate) fn hide_main_window(app_handle: &AppHandle) -> AppResult<()> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .hide()
            .map_err(|e| AppError::message(e.to_string()))?;
    }
    Ok(())
}

pub(crate) async fn exit_application(
    queue_manager: Arc<QueueManager>,
    task_runner: Arc<TaskRunner>,
    app_handle: AppHandle,
) -> AppResult<()> {
    if let Err(err) = queue_manager.prepare_for_exit().await {
        eprintln!("Failed to persist queue state before exit: {}", err);
    }
    if let Err(err) = task_runner.terminate_all_running_processes().await {
        eprintln!("Failed to terminate running processes during exit: {}", err);
    }
    app_handle.exit(0);
    Ok(())
}

fn emit_task_error(app_handle: &AppHandle, task_id: &str, message: String) {
    let payload = serde_json::json!({
        "id": task_id,
        "message": message,
    });
    let _ = app_handle.emit("task-error", payload);
}

pub(crate) fn resolve_close_action(
    behavior: CloseButtonBehavior,
    source: CloseRequestSource,
) -> CloseAction {
    match source {
        CloseRequestSource::TrayQuit => CloseAction::ExitApplication,
        CloseRequestSource::WindowButton => match behavior {
            CloseButtonBehavior::CloseToTray => CloseAction::HideToTray,
            CloseButtonBehavior::Exit => CloseAction::ExitApplication,
        },
    }
}

pub(crate) fn open_download_dir_for_path(path: PathBuf) -> AppResult<()> {
    std::fs::create_dir_all(&path)?;

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::message(format!("Failed to open download directory: {e}")))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::message(format!("Failed to open download directory: {e}")))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::message(format!("Failed to open download directory: {e}")))?;
    }

    Ok(())
}
