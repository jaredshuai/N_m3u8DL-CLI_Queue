mod history_store;
mod models;
mod persistence;
mod queue_manager;
mod task_runner;
#[cfg(test)]
mod test_support;

use history_store::HistoryStore;
use models::{AddTaskPayload, HistoryPage, HistoryStatus, QueueState, Task};
use queue_manager::QueueManager;
use std::sync::Arc;
use task_runner::{StopTaskError, TaskRunner};
use tauri::{Emitter, Listener, Manager};

/// State wrapper for Tauri managed state
pub struct AppState {
    history_store: Arc<HistoryStore>,
    queue_manager: Arc<QueueManager>,
    task_runner: Arc<TaskRunner>,
}

#[tauri::command]
async fn get_queue_state(state: tauri::State<'_, AppState>) -> Result<QueueState, String> {
    Ok(state.queue_manager.get_state().await)
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
    let payload = AddTaskPayload {
        url,
        save_name,
        headers,
    };
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
    task_ids: Vec<String>,
) -> Result<(), String> {
    state.queue_manager.reorder_tasks(task_ids).await
}

#[tauri::command]
async fn start_queue(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    state.queue_manager.set_running(true).await;
    try_schedule_next(
        &state.queue_manager,
        &state.history_store,
        &state.task_runner,
        &app_handle,
    )
    .await;
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

/// Try to schedule the next waiting task for execution
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

/// Handle task-completed event: update queue state and schedule next
async fn handle_task_completed(
    history_store: Arc<HistoryStore>,
    queue_manager: Arc<QueueManager>,
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
}

/// Handle task-failed event: update queue state and schedule next
async fn handle_task_failed(
    history_store: Arc<HistoryStore>,
    queue_manager: Arc<QueueManager>,
    task_runner: Arc<TaskRunner>,
    app_handle: tauri::AppHandle,
    task_id: String,
    error_message: String,
) {
    if let Some(task) = queue_manager.on_task_failed(&task_id, &error_message).await {
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
}

pub fn run() {
    let history_path = HistoryStore::default_path();
    let persistence_path = persistence::Persistence::default_path();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            let queue_manager = Arc::new(QueueManager::new(persistence_path));
            let task_runner = Arc::new(TaskRunner::new());
            let history_store = Arc::new(HistoryStore::new(history_path));

            let state = AppState {
                history_store: Arc::clone(&history_store),
                queue_manager: Arc::clone(&queue_manager),
                task_runner: Arc::clone(&task_runner),
            };

            app.manage(state);

            // Listen for task-completed events
            let hs_completed = Arc::clone(&history_store);
            let qm_completed = Arc::clone(&queue_manager);
            let tr_completed = Arc::clone(&task_runner);
            let ah_completed = app_handle.clone();
            app.listen("task-completed", move |event: tauri::Event| {
                let hs = Arc::clone(&hs_completed);
                let qm = Arc::clone(&qm_completed);
                let tr = Arc::clone(&tr_completed);
                let ah = ah_completed.clone();
                let payload = event.payload().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&payload) {
                        let task_id = data["id"].as_str().unwrap_or("").to_string();
                        let output_path = data["outputPath"].as_str().unwrap_or("").to_string();
                        handle_task_completed(hs, qm, tr, ah, task_id, output_path).await;
                    }
                });
            });

            // Listen for task-failed events
            let hs_failed = Arc::clone(&history_store);
            let qm_failed = Arc::clone(&queue_manager);
            let tr_failed = Arc::clone(&task_runner);
            let ah_failed = app_handle.clone();
            app.listen("task-failed", move |event: tauri::Event| {
                let hs = Arc::clone(&hs_failed);
                let qm = Arc::clone(&qm_failed);
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
                        handle_task_failed(hs, qm, tr, ah, task_id, error_message).await;
                    }
                });
            });

            // Listen for task-progress events
            let qm_progress = Arc::clone(&queue_manager);
            app.listen("task-progress", move |event: tauri::Event| {
                let qm = Arc::clone(&qm_progress);
                let payload = event.payload().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&payload) {
                        let task_id = data["id"].as_str().unwrap_or("").to_string();
                        let progress = data["progress"].as_f64().unwrap_or(-1.0) as f32;
                        let speed = data["speed"].as_str().unwrap_or("").to_string();
                        let threads = data["threads"].as_str().unwrap_or("").to_string();
                        qm.update_task_progress(&task_id, progress, speed, threads)
                            .await;
                    }
                });
            });

            // Listen for task-log events
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
            get_history_page,
            add_task,
            remove_task,
            retry_task,
            reorder_tasks,
            start_queue,
            pause_queue,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::spawn_sleeping_child;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_persistence_path() -> PathBuf {
        std::env::temp_dir().join(format!("queue-state-{}.json", Uuid::new_v4()))
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

    #[tokio::test]
    async fn pause_current_task_resets_state_after_successful_stop() {
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

        pause_current_task(&queue_manager, &task_runner)
            .await
            .expect("pause succeeds");

        let state = queue_manager.get_state().await;
        let paused_task = state
            .tasks
            .iter()
            .find(|t| t.id == task.id)
            .expect("task exists");

        assert_eq!(paused_task.status, models::TaskStatus::Waiting);
        assert!(state.current_task_id.is_none());
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
        queue_manager.on_task_failed(&task.id, "first").await;
        queue_manager.schedule_next().await.expect("rescheduled task");
        queue_manager.on_task_failed(&task.id, "second").await;
        queue_manager.schedule_next().await.expect("rescheduled task");

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
}
