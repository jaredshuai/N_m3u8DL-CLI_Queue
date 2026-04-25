mod app_error;
mod app_state;
mod cli_output_store;
mod commands;
mod download_dir;
mod event_handlers;
mod history_store;
mod models;
mod persistence;
mod queue_manager;
mod runtime;
mod settings_store;
mod shutdown;
mod task_runner;
#[cfg(test)]
mod test_support;
mod tray;

use app_state::AppState;
use cli_output_store::CliOutputStore;
use commands::{
    add_task, cancel_auto_shutdown, get_app_settings, get_cli_output_page, get_cli_output_tail,
    get_cli_terminal_state, get_history_page, get_queue_state, minimize_main_window,
    open_download_dir, pause_queue, remove_history_task, remove_task, reorder_tasks,
    request_main_window_close, retry_task, start_queue, toggle_main_window_maximize,
    update_app_settings,
};
use history_store::HistoryStore;
use queue_manager::QueueManager;
use runtime::CloseRequestSource;
use settings_store::SettingsStore;
use shutdown::ShutdownManager;
use task_runner::TaskRunner;
use tauri::{Manager, WindowEvent};

fn handle_main_window_close_requested<F>(window_label: &str, mut request_close: F) -> bool
where
    F: FnMut(CloseRequestSource),
{
    if window_label != "main" {
        return false;
    }

    request_close(CloseRequestSource::WindowButton);
    true
}

pub fn run() {
    let cli_output_path = CliOutputStore::default_path();
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
                if handle_main_window_close_requested(window.label(), |source| {
                    tray::request_close_from_handle(window.app_handle().clone(), source);
                }) {
                    api.prevent_close();
                }
            }
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            let state = AppState::new(
                std::sync::Arc::new(CliOutputStore::new(cli_output_path)),
                std::sync::Arc::new(HistoryStore::new(history_path)),
                std::sync::Arc::new(QueueManager::new(persistence_path)),
                std::sync::Arc::new(SettingsStore::new(settings_path)),
                std::sync::Arc::new(ShutdownManager::new()),
                std::sync::Arc::new(TaskRunner::new()),
            );

            app.manage(state.clone());
            tray::setup_tray(app)?;
            event_handlers::register_event_handlers(app, app_handle, state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_queue_state,
            get_app_settings,
            update_app_settings,
            get_history_page,
            get_cli_output_tail,
            get_cli_output_page,
            get_cli_terminal_state,
            add_task,
            remove_task,
            remove_history_task,
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
    use crate::history_store::HistoryStore;
    use crate::models::{AddTaskPayload, AppSettings, CloseButtonBehavior, HistoryStatus};
    use crate::queue_manager::{QueueManager, TaskFailureTransition};
    use crate::runtime::{
        handle_start_failure, maybe_start_shutdown_countdown, pause_queue_internal,
        resolve_close_action, CloseAction, CloseRequestSource,
    };
    use crate::shutdown::ShutdownManager;
    use crate::task_runner::TaskRunner;
    use crate::test_support::spawn_sleeping_child;
    use std::path::PathBuf;
    use std::sync::Arc;
    use uuid::Uuid;

    fn temp_persistence_path() -> PathBuf {
        std::env::temp_dir().join(format!("queue-state-{}.json", Uuid::new_v4()))
    }

    #[test]
    fn tray_quit_always_exits_even_when_window_close_hides_to_tray() {
        assert_eq!(
            resolve_close_action(
                CloseButtonBehavior::CloseToTray,
                CloseRequestSource::TrayQuit,
            ),
            CloseAction::ExitApplication
        );
    }

    #[test]
    fn window_close_respects_close_to_tray_setting() {
        assert_eq!(
            resolve_close_action(
                CloseButtonBehavior::CloseToTray,
                CloseRequestSource::WindowButton,
            ),
            CloseAction::HideToTray
        );
        assert_eq!(
            resolve_close_action(CloseButtonBehavior::Exit, CloseRequestSource::WindowButton),
            CloseAction::ExitApplication
        );
    }

    #[test]
    fn native_window_close_is_wired_as_window_button_source() {
        let mut requested_source = None;

        let handled = super::handle_main_window_close_requested("main", |source| {
            requested_source = Some(source);
        });

        assert!(handled);
        assert_eq!(requested_source, Some(CloseRequestSource::WindowButton));
    }

    #[test]
    fn non_main_window_close_is_not_handled_by_main_close_wiring() {
        let mut requested_source = None;

        let handled = super::handle_main_window_close_requested("secondary", |source| {
            requested_source = Some(source);
        });

        assert!(!handled);
        assert_eq!(requested_source, None);
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
            download_dir: None,
        };
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = queue_manager.add_task(payload).await;

        queue_manager.set_running(true).await;
        queue_manager.schedule_next().await.expect("scheduled task");

        let started = maybe_start_shutdown_countdown(&queue_manager, &shutdown_manager, &settings)
            .await
            .expect("countdown check succeeds");
        assert!(started.is_none());

        let completed_task = queue_manager
            .snapshot_task_completion(&task.id, "D:/Videos/test.mp4")
            .await
            .expect("task completed");
        history_store
            .append(&completed_task)
            .expect("append completed task");
        assert!(queue_manager.finalize_task_completion(&task.id).await);

        let started = maybe_start_shutdown_countdown(&queue_manager, &shutdown_manager, &settings)
            .await
            .expect("countdown check succeeds");

        assert_eq!(started, Some(crate::shutdown::shutdown_seconds()));
        std::fs::remove_dir_all(history_path).expect("cleanup history");
    }

    #[tokio::test]
    async fn pause_queue_internal_leaves_current_download_running() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let task_runner = Arc::new(TaskRunner::new());
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = queue_manager.add_task(payload).await;
        let child = spawn_sleeping_child().await;

        queue_manager.set_running(true).await;
        queue_manager.schedule_next().await.expect("scheduled task");
        task_runner
            .insert_running_task_for_test(task.id.clone(), child)
            .await;
        task_runner.begin_wait_for_test(&task.id).await;

        pause_queue_internal(&queue_manager)
            .await
            .expect("pause queue succeeds");

        let state = queue_manager.get_state().await;
        let active_task = state
            .tasks
            .iter()
            .find(|t| t.id == task.id)
            .expect("task exists");

        assert!(!state.is_running);
        assert_eq!(state.current_task_id.as_deref(), Some(task.id.as_str()));
        assert_eq!(active_task.status, crate::models::TaskStatus::Downloading);
        assert!(task_runner.is_task_running(&task.id).await);
    }

    #[tokio::test]
    async fn handle_start_failure_persists_terminal_task_to_history() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = queue_manager.add_task(payload).await;

        queue_manager.set_running(true).await;
        queue_manager.schedule_next().await.expect("scheduled task");
        assert!(matches!(
            queue_manager.prepare_task_failure(&task.id, "first").await,
            Some(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("rescheduled task");
        assert!(matches!(
            queue_manager.prepare_task_failure(&task.id, "second").await,
            Some(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("rescheduled task");

        handle_start_failure(&queue_manager, &history_store, &task.id, "third")
            .await
            .expect("persist terminal failure");

        let state = queue_manager.get_state().await;
        assert!(state.tasks.is_empty());
        let page = history_store
            .get_page(HistoryStatus::Failed, 0, 20)
            .expect("history page");
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.tasks[0].id, task.id);

        std::fs::remove_dir_all(history_path).expect("cleanup history");
    }

    #[tokio::test]
    async fn handle_start_failure_keeps_task_when_history_append_fails() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        std::fs::write(&history_path, b"blocked").expect("create blocking file");
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = queue_manager.add_task(payload).await;

        queue_manager.set_running(true).await;
        queue_manager.schedule_next().await.expect("scheduled task");
        assert!(matches!(
            queue_manager.prepare_task_failure(&task.id, "first").await,
            Some(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("rescheduled task");
        assert!(matches!(
            queue_manager.prepare_task_failure(&task.id, "second").await,
            Some(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("rescheduled task");

        let result = handle_start_failure(&queue_manager, &history_store, &task.id, "third").await;
        assert!(result.is_err());

        let state = queue_manager.get_state().await;
        assert!(state.tasks.iter().any(|t| t.id == task.id));
        assert_eq!(state.current_task_id.as_deref(), Some(task.id.as_str()));

        let _ = std::fs::remove_file(history_path);
    }

    #[tokio::test]
    async fn completed_task_stays_in_queue_when_history_append_fails() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        std::fs::write(&history_path, b"blocked").expect("create blocking file");
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = queue_manager.add_task(payload).await;

        queue_manager.set_running(true).await;
        queue_manager.schedule_next().await.expect("scheduled task");

        let completed_task = queue_manager
            .snapshot_task_completion(&task.id, "D:/Videos/test.mp4")
            .await
            .expect("task completed");
        let append_result = history_store.append(&completed_task);
        assert!(append_result.is_err());

        let state = queue_manager.get_state().await;
        assert!(state.tasks.iter().any(|t| t.id == task.id));
        assert_eq!(state.current_task_id.as_deref(), Some(task.id.as_str()));

        let _ = std::fs::remove_file(history_path);
    }
}
