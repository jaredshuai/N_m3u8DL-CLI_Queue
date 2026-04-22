mod history_store;
mod models;
mod persistence;
mod queue_manager;
mod settings_store;
mod shutdown;
mod task_runner;
#[cfg(test)]
mod test_support;

use history_store::HistoryStore;
use models::{AddTaskPayload, AppSettings, CloseButtonBehavior, HistoryPage, HistoryStatus, QueueState, Task};
use queue_manager::QueueManager;
use settings_store::SettingsStore;
use shutdown::ShutdownManager;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use task_runner::{StopTaskError, TaskRunner};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Listener, Manager, WindowEvent};

const DOWNLOAD_DIR: &str = r"D:\Videos";

pub struct AppState {
    history_store: Arc<HistoryStore>,
    queue_manager: Arc<QueueManager>,
    settings_store: Arc<SettingsStore>,
    shutdown_manager: Arc<ShutdownManager>,
    task_runner: Arc<TaskRunner>,
}

#[tauri::command]
async fn get_queue_state(state: tauri::State<'_, AppState>) -> Result<QueueState, String> {
    Ok(state.queue_manager.get_state().await)
}

#[tauri::command]
fn get_app_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings_store.get())
}

#[tauri::command]
fn update_app_settings(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, String> {
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
async fn get_history_page(
    state: tauri::State<'_, AppState>,
    status: HistoryStatus,
    offset: usize,
    limit: usize,
) -> Result<HistoryPage, String> {
    state.history_store.get_page(status, offset, limit)
}

#[tauri::command]
async fn add_task(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    url: String,
    save_name: Option<String>,
    headers: Option<String>,
) -> Result<Task, String> {
    let payload = AddTaskPayload { url, save_name, headers };
    let (task, should_schedule) = state.queue_manager.add_task(payload).await;
    if should_schedule {
        try_schedule_next(
            &state.queue_manager,
            &state.history_store,
            &state.task_runner,
            &app_handle,
        )
        .await;
    }
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(task)
}

#[tauri::command]
async fn remove_task(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    task_id: String,
) -> Result<(), String> {
    state.queue_manager.remove_task(&task_id).await?;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(())
}

#[tauri::command]
async fn retry_task(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    task_id: String,
) -> Result<Task, String> {
    let task = match state.queue_manager.retry_task(&task_id).await {
        Ok(task) => task,
        Err(active_error) if active_error.contains("not found") => {
            let history_task = state
                .history_store
                .find_task(HistoryStatus::Failed, &task_id)?
                .ok_or(active_error)?;
            let (task, should_schedule) = state.queue_manager.add_history_retry_task(&history_task).await;
            if should_schedule {
                try_schedule_next(
                    &state.queue_manager,
                    &state.history_store,
                    &state.task_runner,
                    &app_handle,
                )
                .await;
            }
            task
        }
        Err(err) => return Err(err),
    };
    try_schedule_next(
        &state.queue_manager,
        &state.history_store,
        &state.task_runner,
        &app_handle,
    )
    .await;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(task)
}

#[tauri::command]
async fn reorder_tasks(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    task_ids: Vec<String>,
) -> Result<(), String> {
    state.queue_manager.reorder_tasks(task_ids).await?;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(())
}

#[tauri::command]
async fn start_queue(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    state.shutdown_manager.reset_run_failure();
    if !state.queue_manager.has_live_work().await {
        state.queue_manager.set_running(false).await;
        let _ = app_handle.emit("queue-state-changed", ());
        return Ok(());
    }

    state.queue_manager.set_running(true).await;
    try_schedule_next(
        &state.queue_manager,
        &state.history_store,
        &state.task_runner,
        &app_handle,
    )
    .await;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(())
}

#[tauri::command]
async fn pause_queue(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    pause_current_task(&state.queue_manager, &state.task_runner).await?;
    state.queue_manager.set_running(false).await;
    let _ = app_handle.emit("queue-state-changed", ());
    Ok(())
}

#[tauri::command]
fn minimize_main_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.minimize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn toggle_main_window_maximize(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        if window.is_maximized().map_err(|e| e.to_string())? {
            window.unmaximize().map_err(|e| e.to_string())?;
        } else {
            window.maximize().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn request_main_window_close(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    apply_close_behavior(&state.settings_store, &app_handle)
}

#[tauri::command]
fn open_download_dir() -> Result<(), String> {
    open_default_download_dir()
}

#[tauri::command]
fn cancel_auto_shutdown(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    state.shutdown_manager.cancel_countdown()?;
    let _ = app_handle.emit("shutdown-countdown-cancelled", ());
    Ok(())
}

async fn pause_current_task(
    queue_manager: &Arc<QueueManager>,
    task_runner: &Arc<TaskRunner>,
) -> Result<(), String> {
    if let Some(task_id) = queue_manager.current_task_id().await {
        match task_runner.stop_task(&task_id).await {
            Ok(()) | Err(StopTaskError::NoRunningProcess(_)) => {}
            Err(err) => return Err(err.to_string()),
        }
        queue_manager.on_task_paused(&task_id).await;
    }
    Ok(())
}

async fn try_schedule_next(
    queue_manager: &Arc<QueueManager>,
    history_store: &Arc<HistoryStore>,
    task_runner: &Arc<TaskRunner>,
    app_handle: &tauri::AppHandle,
) {
    loop {
        let task = match queue_manager.schedule_next().await {
            Some(t) => t,
            None => return,
        };
        let task_id = task.id.clone();
        match task_runner.start_task(task, app_handle.clone()).await {
            Ok(()) => return,
            Err(e) => {
                eprintln!("Failed to start task {}: {}", task_id, e);
                match handle_start_failure(queue_manager, history_store, &task_id, &e).await {
                    Ok(Some(task)) => {
                        let payload = serde_json::json!({
                            "status": HistoryStatus::Failed,
                            "task": task,
                        });
                        let _ = app_handle.emit("history-task-added", payload);
                    }
                    Ok(None) => {}
                    Err(err) => eprintln!("Failed to persist terminal start failure: {}", err),
                }
            }
        }
    }
}

async fn handle_start_failure(
    queue_manager: &Arc<QueueManager>,
    history_store: &Arc<HistoryStore>,
    task_id: &str,
    error_message: &str,
) -> Result<Option<Task>, String> {
    if let Some(task) = queue_manager.on_task_failed(task_id, error_message).await {
        history_store.append(&task)?;
        return Ok(Some(task));
    }
    Ok(None)
}

async fn handle_task_completed(
    history_store: Arc<HistoryStore>,
    queue_manager: Arc<QueueManager>,
    settings_store: Arc<SettingsStore>,
    shutdown_manager: Arc<ShutdownManager>,
    task_runner: Arc<TaskRunner>,
    app_handle: tauri::AppHandle,
    task_id: String,
    output_path: String,
) {
    if let Some(task) = queue_manager.on_task_completed(&task_id, &output_path).await {
        if let Err(err) = history_store.append(&task) {
            eprintln!("Failed to append completed task to history: {}", err);
        } else {
            let payload = serde_json::json!({
                "status": HistoryStatus::Completed,
                "task": task,
            });
            let _ = app_handle.emit("history-task-added", payload);
        }
    }
    let _ = app_handle.emit("queue-state-changed", ());
    try_schedule_next(&queue_manager, &history_store, &task_runner, &app_handle).await;
    if queue_manager.finish_run_if_idle().await {
        let _ = app_handle.emit("queue-state-changed", ());
        if let Ok(Some(seconds)) = maybe_start_shutdown_countdown(
            &queue_manager,
            &shutdown_manager,
            &settings_store.get(),
        )
        .await
        {
            let payload = serde_json::json!({ "seconds": seconds });
            let _ = app_handle.emit("shutdown-countdown-started", payload);
        }
    }
}

async fn handle_task_failed(
    history_store: Arc<HistoryStore>,
    queue_manager: Arc<QueueManager>,
    settings_store: Arc<SettingsStore>,
    shutdown_manager: Arc<ShutdownManager>,
    task_runner: Arc<TaskRunner>,
    app_handle: tauri::AppHandle,
    task_id: String,
    error_message: String,
) {
    if let Some(task) = queue_manager.on_task_failed(&task_id, &error_message).await {
        shutdown_manager.mark_run_failure();
        if let Err(err) = history_store.append(&task) {
            eprintln!("Failed to append failed task to history: {}", err);
        } else {
            let payload = serde_json::json!({
                "status": HistoryStatus::Failed,
                "task": task,
            });
            let _ = app_handle.emit("history-task-added", payload);
        }
    }
    let _ = app_handle.emit("queue-state-changed", ());
    try_schedule_next(&queue_manager, &history_store, &task_runner, &app_handle).await;
    if queue_manager.finish_run_if_idle().await {
        let _ = app_handle.emit("queue-state-changed", ());
        if let Ok(Some(seconds)) = maybe_start_shutdown_countdown(
            &queue_manager,
            &shutdown_manager,
            &settings_store.get(),
        )
        .await
        {
            let payload = serde_json::json!({ "seconds": seconds });
            let _ = app_handle.emit("shutdown-countdown-started", payload);
        }
    }
}

async fn maybe_start_shutdown_countdown(
    queue_manager: &Arc<QueueManager>,
    shutdown_manager: &Arc<ShutdownManager>,
    settings: &AppSettings,
) -> Result<Option<u64>, String> {
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

fn show_main_window(app_handle: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.unminimize().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn hide_main_window(app_handle: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn apply_close_behavior(
    settings_store: &Arc<SettingsStore>,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    match settings_store.get().close_button_behavior {
        CloseButtonBehavior::CloseToTray => hide_main_window(app_handle),
        CloseButtonBehavior::Exit => {
            app_handle.exit(0);
            Ok(())
        }
    }
}

fn open_default_download_dir() -> Result<(), String> {
    let path = PathBuf::from(DOWNLOAD_DIR);
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open download directory: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open download directory: {e}"))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open download directory: {e}"))?;
    }

    Ok(())
}

fn start_queue_from_handle(app_handle: tauri::AppHandle) {
    let state = app_handle.state::<AppState>();
    let queue_manager = Arc::clone(&state.queue_manager);
    let history_store = Arc::clone(&state.history_store);
    let shutdown_manager = Arc::clone(&state.shutdown_manager);
    let task_runner = Arc::clone(&state.task_runner);

    tauri::async_runtime::spawn(async move {
        shutdown_manager.reset_run_failure();
        if !queue_manager.has_live_work().await {
            queue_manager.set_running(false).await;
            let _ = app_handle.emit("queue-state-changed", ());
            return;
        }
        queue_manager.set_running(true).await;
        try_schedule_next(&queue_manager, &history_store, &task_runner, &app_handle).await;
        let _ = app_handle.emit("queue-state-changed", ());
    });
}

fn pause_queue_from_handle(app_handle: tauri::AppHandle) {
    let state = app_handle.state::<AppState>();
    let queue_manager = Arc::clone(&state.queue_manager);
    let task_runner = Arc::clone(&state.task_runner);

    tauri::async_runtime::spawn(async move {
        if let Err(err) = pause_current_task(&queue_manager, &task_runner).await {
            eprintln!("Failed to pause queue from tray: {}", err);
            return;
        }
        queue_manager.set_running(false).await;
        let _ = app_handle.emit("queue-state-changed", ());
    });
}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show-main-window", "Show main window", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "start-queue", "Start queue", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause-queue", "Pause queue", true, None::<&str>)?;
    let open_dir = MenuItem::with_id(app, "open-download-dir", "Open download directory", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &start, &pause, &open_dir, &quit])?;

    let icon = app.default_window_icon().cloned();
    let mut tray_builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("m3u8 Queue Downloader")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show-main-window" => {
                let _ = show_main_window(app);
            }
            "start-queue" => start_queue_from_handle(app.clone()),
            "pause-queue" => pause_queue_from_handle(app.clone()),
            "open-download-dir" => {
                if let Err(err) = open_default_download_dir() {
                    eprintln!("Failed to open download directory from tray: {}", err);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = icon {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder.build(app)?;
    Ok(())
}

pub fn run() {
    let history_path = HistoryStore::default_path();
    let persistence_path = persistence::Persistence::default_path();
    let settings_path = SettingsStore::default_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let state = window.state::<AppState>();
                if let Err(err) = apply_close_behavior(&state.settings_store, window.app_handle()) {
                    eprintln!("Failed to apply close behavior: {}", err);
                }
            }
        })
        .setup(|app| {
            let app_handle = app.handle().clone();

            let queue_manager = Arc::new(QueueManager::new(persistence_path));
            let task_runner = Arc::new(TaskRunner::new());
            let history_store = Arc::new(HistoryStore::new(history_path));
            let settings_store = Arc::new(SettingsStore::new(settings_path));
            let shutdown_manager = Arc::new(ShutdownManager::new());

            let state = AppState {
                history_store: Arc::clone(&history_store),
                queue_manager: Arc::clone(&queue_manager),
                settings_store: Arc::clone(&settings_store),
                shutdown_manager: Arc::clone(&shutdown_manager),
                task_runner: Arc::clone(&task_runner),
            };

            app.manage(state);
            setup_tray(app)?;

            let hs_completed = Arc::clone(&history_store);
            let qm_completed = Arc::clone(&queue_manager);
            let ss_completed = Arc::clone(&settings_store);
            let sm_completed = Arc::clone(&shutdown_manager);
            let tr_completed = Arc::clone(&task_runner);
            let ah_completed = app_handle.clone();
            app.listen("task-completed", move |event: tauri::Event| {
                let hs = Arc::clone(&hs_completed);
                let qm = Arc::clone(&qm_completed);
                let ss = Arc::clone(&ss_completed);
                let sm = Arc::clone(&sm_completed);
                let tr = Arc::clone(&tr_completed);
                let ah = ah_completed.clone();
                let payload = event.payload().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&payload) {
                        let task_id = data["id"].as_str().unwrap_or("").to_string();
                        let output_path = data["outputPath"].as_str().unwrap_or("").to_string();
                        handle_task_completed(hs, qm, ss, sm, tr, ah, task_id, output_path).await;
                    }
                });
            });

            let hs_failed = Arc::clone(&history_store);
            let qm_failed = Arc::clone(&queue_manager);
            let ss_failed = Arc::clone(&settings_store);
            let sm_failed = Arc::clone(&shutdown_manager);
            let tr_failed = Arc::clone(&task_runner);
            let ah_failed = app_handle.clone();
            app.listen("task-failed", move |event: tauri::Event| {
                let hs = Arc::clone(&hs_failed);
                let qm = Arc::clone(&qm_failed);
                let ss = Arc::clone(&ss_failed);
                let sm = Arc::clone(&sm_failed);
                let tr = Arc::clone(&tr_failed);
                let ah = ah_failed.clone();
                let payload = event.payload().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&payload) {
                        let task_id = data["id"].as_str().unwrap_or("").to_string();
                        let error_message = data["errorMessage"]
                            .as_str()
                            .unwrap_or("Unknown error")
                            .to_string();
                        handle_task_failed(hs, qm, ss, sm, tr, ah, task_id, error_message).await;
                    }
                });
            });

            let qm_progress = Arc::clone(&queue_manager);
            app.listen("task-progress", move |event: tauri::Event| {
                let qm = Arc::clone(&qm_progress);
                let payload = event.payload().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&payload) {
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
                        qm.update_task_progress(&task_id, progress, speed, threads).await;
                    }
                });
            });

            let qm_log = Arc::clone(&queue_manager);
            app.listen("task-log", move |event: tauri::Event| {
                let qm = Arc::clone(&qm_log);
                let payload = event.payload().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&payload) {
                        let task_id = data["id"].as_str().unwrap_or("").to_string();
                        let line = data["line"].as_str().unwrap_or("").to_string();
                        qm.append_log(&task_id, line).await;
                    }
                });
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_queue_state,
            get_app_settings,
            update_app_settings,
            get_history_page,
            add_task,
            remove_task,
            retry_task,
            reorder_tasks,
            start_queue,
            pause_queue,
            minimize_main_window,
            toggle_main_window_maximize,
            request_main_window_close,
            open_download_dir,
            cancel_auto_shutdown,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shutdown::ShutdownManager;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_persistence_path() -> PathBuf {
        std::env::temp_dir().join(format!("queue-state-{}.json", Uuid::new_v4()))
    }

    #[tokio::test]
    async fn completed_run_with_auto_shutdown_enabled_starts_countdown() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let shutdown_manager = Arc::new(ShutdownManager::new());
        let settings = AppSettings {
            close_button_behavior: CloseButtonBehavior::CloseToTray,
            auto_action_on_complete: true,
        };
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = queue_manager.add_task(payload).await;

        queue_manager.set_running(true).await;
        queue_manager.schedule_next().await.expect("scheduled task");

        let started = maybe_start_shutdown_countdown(
            &queue_manager,
            &shutdown_manager,
            &settings,
        )
        .await
        .expect("countdown check succeeds");
        assert!(started.is_none());

        let completed_task = queue_manager
            .on_task_completed(&task.id, "D:/Videos/test.mp4")
            .await
            .expect("task completed");
        history_store
            .append(&completed_task)
            .expect("append completed task");

        let started = maybe_start_shutdown_countdown(
            &queue_manager,
            &shutdown_manager,
            &settings,
        )
        .await
        .expect("countdown check succeeds");

        assert_eq!(started, Some(crate::shutdown::shutdown_seconds()));
        std::fs::remove_dir_all(history_path).expect("cleanup history");
    }

    #[tokio::test]
    async fn pause_current_task_recovers_when_process_is_missing() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let task_runner = Arc::new(TaskRunner::new());
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = queue_manager.add_task(payload).await;

        queue_manager.set_running(true).await;
        queue_manager.schedule_next().await.expect("scheduled task");

        pause_current_task(&queue_manager, &task_runner)
            .await
            .expect("missing process should be recovered");

        let state = queue_manager.get_state().await;
        let paused_task = state
            .tasks
            .iter()
            .find(|t| t.id == task.id)
            .expect("task exists");

        assert_eq!(paused_task.status, models::TaskStatus::Waiting);
        assert!(state.current_task_id.is_none());
    }
}
