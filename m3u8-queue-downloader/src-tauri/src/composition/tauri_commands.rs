use crate::adapters::frontend_dto::{
    CliOutputPageDto, CliTerminalStateDto, HistoryPageDto, QueueStateDto, TaskDto,
};
use crate::adapters::history_status_codec::parse_history_status;
use crate::adapters::settings_dto::AppSettingsDto;
use crate::adapters::tauri_frontend_event_publisher::TauriFrontendEventPublisher;
use crate::adapters::window_actions;
use crate::application::app_error::{AppError, AppResult};
use crate::application::close_policy::CloseRequestSource;
use crate::application::queue_requests::AddTaskPayload;
use crate::composition::dependency_graph::DependencyGraph;
use crate::composition::download_directory_command_facade::DownloadDirectoryCommandFacade;
use crate::composition::exit_command_facade::ExitCommandFacade;
use crate::composition::history_command_facade::HistoryCommandFacade;
use crate::composition::queue_command_facade::QueueCommandFacade;
use crate::composition::read_model_query_facade::ReadModelQueryFacade;
use crate::composition::settings_command_facade::SettingsCommandFacade;
use tauri::State;

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
pub async fn get_queue_state(state: State<'_, DependencyGraph>) -> Result<QueueStateDto, String> {
    let queries = ReadModelQueryFacade::new(state.inner().clone());
    Ok(queries.get_queue_state().await.into())
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, DependencyGraph>) -> Result<AppSettingsDto, String> {
    let queries = ReadModelQueryFacade::new(state.inner().clone());
    Ok(queries.get_app_settings().into())
}

#[tauri::command]
pub fn update_app_settings(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
    settings: AppSettingsDto,
) -> Result<AppSettingsDto, String> {
    let events = TauriFrontendEventPublisher::new(app_handle);
    let settings_commands = SettingsCommandFacade::new(state.inner().clone());
    command_result(settings_commands.update_app_settings(&events, settings.into()))
        .map(AppSettingsDto::from)
}

#[tauri::command]
pub async fn get_history_page(
    state: State<'_, DependencyGraph>,
    status: String,
    offset: usize,
    limit: usize,
) -> Result<HistoryPageDto, String> {
    let status = command_result(parse_history_status(&status))?;
    let queries = ReadModelQueryFacade::new(state.inner().clone());
    let query = queries.history_page_query(status, offset, limit).await;
    command_result(spawn_blocking_result("history page task failed to join", query).await)
        .map(HistoryPageDto::from)
}

#[tauri::command]
pub async fn get_cli_output_tail(
    state: State<'_, DependencyGraph>,
    task_id: String,
    limit: usize,
) -> Result<CliOutputPageDto, String> {
    let queries = ReadModelQueryFacade::new(state.inner().clone());
    command_result(
        spawn_blocking_result(
            "cli output tail task failed to join",
            queries.cli_output_tail_query(task_id, limit),
        )
        .await,
    )
    .map(CliOutputPageDto::from)
}

#[tauri::command]
pub async fn get_cli_output_page(
    state: State<'_, DependencyGraph>,
    task_id: String,
    offset: usize,
    limit: usize,
) -> Result<CliOutputPageDto, String> {
    let queries = ReadModelQueryFacade::new(state.inner().clone());
    command_result(
        spawn_blocking_result(
            "cli output page task failed to join",
            queries.cli_output_page_query(task_id, offset, limit),
        )
        .await,
    )
    .map(CliOutputPageDto::from)
}

#[tauri::command]
pub async fn get_cli_terminal_state(
    state: State<'_, DependencyGraph>,
    task_id: String,
    limit: usize,
) -> Result<CliTerminalStateDto, String> {
    let queries = ReadModelQueryFacade::new(state.inner().clone());
    command_result(
        spawn_blocking_result(
            "cli terminal state task failed to join",
            queries.cli_terminal_state_query(task_id, limit),
        )
        .await,
    )
    .map(CliTerminalStateDto::from)
}

#[tauri::command]
pub async fn add_task(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
    url: String,
    save_name: Option<String>,
    headers: Option<String>,
) -> Result<TaskDto, String> {
    let payload = AddTaskPayload {
        url,
        save_name,
        headers,
    };
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let queue_commands = QueueCommandFacade::new(state.inner().clone());
    command_result(queue_commands.add_task(&events, payload).await).map(TaskDto::from)
}

#[tauri::command]
pub async fn remove_task(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
    task_id: String,
) -> Result<(), String> {
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let queue_commands = QueueCommandFacade::new(state.inner().clone());
    command_result(queue_commands.remove_task(&events, &task_id).await)
}

#[tauri::command]
pub async fn update_save_name(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
    task_id: String,
    save_name: Option<String>,
) -> Result<(), String> {
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let queue_commands = QueueCommandFacade::new(state.inner().clone());
    command_result(
        queue_commands
            .update_save_name(&events, &task_id, save_name)
            .await,
    )
}

#[tauri::command]
pub async fn remove_history_task(
    state: State<'_, DependencyGraph>,
    status: String,
    task_id: String,
) -> Result<(), String> {
    let status = command_result(parse_history_status(&status))?;
    let history_commands = HistoryCommandFacade::new(state.inner().clone());
    command_result(history_commands.remove_history_task(status, &task_id))
}

#[tauri::command]
pub async fn retry_task(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
    task_id: String,
) -> Result<TaskDto, String> {
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let queue_commands = QueueCommandFacade::new(state.inner().clone());
    command_result(queue_commands.retry_task(&events, &task_id).await).map(TaskDto::from)
}

#[tauri::command]
pub async fn reorder_tasks(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
    task_ids: Vec<String>,
) -> Result<(), String> {
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let queue_commands = QueueCommandFacade::new(state.inner().clone());
    command_result(queue_commands.reorder_tasks(&events, task_ids).await)
}

#[tauri::command]
pub async fn start_queue(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let queue_commands = QueueCommandFacade::new(state.inner().clone());
    command_result(queue_commands.start_queue(&events).await)
}

#[tauri::command]
pub async fn pause_queue(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let queue_commands = QueueCommandFacade::new(state.inner().clone());
    command_result(queue_commands.pause_queue(&events).await)
}

#[tauri::command]
pub async fn stop_task(
    state: State<'_, DependencyGraph>,
    task_id: String,
) -> Result<(), String> {
    let queue_commands = QueueCommandFacade::new(state.inner().clone());
    command_result(queue_commands.stop_task(&task_id).await)
}

#[tauri::command]
pub fn minimize_main_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    command_result(window_actions::minimize_main_window(&app_handle))
}

#[tauri::command]
pub fn toggle_main_window_maximize(app_handle: tauri::AppHandle) -> Result<(), String> {
    command_result(window_actions::toggle_main_window_maximize(&app_handle))
}

#[tauri::command]
pub async fn request_main_window_close(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let exit_commands = ExitCommandFacade::new(state.inner().clone());
    command_result(
        exit_commands
            .request_close(&events, CloseRequestSource::WindowButton)
            .await,
    )
}

#[tauri::command]
pub fn open_download_dir(state: State<'_, DependencyGraph>) -> Result<(), String> {
    let download_directory_commands = DownloadDirectoryCommandFacade::new(state.inner().clone());
    command_result(download_directory_commands.open_download_dir())
}

#[tauri::command]
pub fn cancel_auto_shutdown(
    state: State<'_, DependencyGraph>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let events = TauriFrontendEventPublisher::new(app_handle.clone());
    let exit_commands = ExitCommandFacade::new(state.inner().clone());
    command_result(exit_commands.cancel_auto_shutdown(&events))
}
