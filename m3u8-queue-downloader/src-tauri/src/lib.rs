mod adapters;
mod application;
mod composition;
mod domain;
mod ports;
#[cfg(test)]
mod test_support;

use composition::app_bootstrap;
use composition::tauri_commands::{
    add_task, cancel_auto_shutdown, get_app_settings, get_cli_output_page, get_cli_output_tail,
    get_cli_terminal_state, get_history_page, get_queue_state, minimize_main_window,
    open_download_dir, pause_queue, remove_history_task, remove_task, reorder_tasks,
    request_main_window_close, retry_task, start_queue, toggle_main_window_maximize,
    update_app_settings, update_save_name,
};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .on_window_event(composition::window_close::handle_window_event)
        .setup(app_bootstrap::setup_app)
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
            update_save_name,
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
    use crate::adapters::cli_output_store::CliOutputStore;
    use crate::adapters::history_store::HistoryStore;
    use crate::adapters::queue_manager::QueueManager;
    use crate::adapters::shutdown::ShutdownManager;
    use crate::adapters::task_runner::TaskRunner;
    use crate::application::app_error::{AppError, AppResult};
    use crate::application::download_directory::DownloadDirectory;
    use crate::application::process_runner_outcomes::ProcessRunnerShutdownStatus;
    use crate::application::queue_mutation_orchestrator::QueueMutationPorts;
    use crate::application::queue_repository_outcomes::{
        PrepareTaskFailureOutcome, QueueRunStatus, TaskFailureTransition,
    };
    use crate::application::queue_requests::AddTaskPayload;
    use crate::application::queue_scheduling_orchestrator::QueueSchedulingPorts;
    use crate::application::settings::{AppSettings, CloseButtonBehavior};
    use crate::application::shutdown_scheduler_outcomes::{
        ShutdownCountdownStartDecision, ShutdownResetOutcome,
    };
    use crate::application::task_lifecycle_orchestrator::TaskLifecyclePorts;
    use crate::application::task_output_event_orchestrator::TaskOutputEventPorts;
    use crate::application::task_process_events::TaskOutputEvent;
    use crate::application::task_process_start_request::TaskProcessStartRequest;
    use crate::application::task_snapshot::TaskSnapshot;
    use crate::application::terminal_history_orchestrator::TerminalHistoryPorts;
    use crate::application::terminal_history_use_cases::{
        flush_pending_history_tasks, handle_completed_task_history,
        handle_terminal_failure_task_history, TerminalHistoryRecordOutcome,
    };
    use crate::application::terminal_output_outcomes::TerminalActiveLine;
    use crate::application::terminal_output_page::TerminalOutputPage;
    use crate::domain::history::HistoryStatus;
    use crate::domain::task::{Task, TaskStatus};
    use crate::ports::diagnostics::Diagnostics;
    use crate::ports::download_directory_resolver::DownloadDirectoryResolver;
    use crate::ports::event_publisher::FrontendEventPublisher;
    use crate::ports::history_repository::HistoryRepository;
    use crate::ports::process_runner::{ProcessRunnerFuture, TaskProcessRunner};
    use crate::ports::queue_repository::QueueRepository;
    use crate::ports::settings_repository::SettingsRepository;
    use crate::ports::shutdown_scheduler::ShutdownScheduler;
    use crate::ports::terminal_output_repository::TerminalOutputRepository;
    use crate::test_support::spawn_sleeping_child;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn temp_persistence_path() -> PathBuf {
        std::env::temp_dir().join(format!("queue-state-{}.json", Uuid::new_v4()))
    }

    fn terminal_history_orchestrator<'a>(
        queue_repository: &'a dyn QueueRepository,
        history_repository: &'a dyn HistoryRepository,
    ) -> TerminalHistoryPorts<'a> {
        TerminalHistoryPorts::new(queue_repository, history_repository)
    }

    fn queue_scheduling_orchestrator<'a>(
        queue_repository: &'a dyn QueueRepository,
        history_repository: &'a dyn HistoryRepository,
    ) -> QueueSchedulingPorts<'a> {
        static PROCESS_RUNNER: NoopProcessRunner = NoopProcessRunner;
        static DIAGNOSTICS: NoopDiagnostics = NoopDiagnostics;
        static EVENTS: NoopFrontendEvents = NoopFrontendEvents;
        queue_scheduling_orchestrator_with_process(
            queue_repository,
            history_repository,
            &PROCESS_RUNNER,
            &DIAGNOSTICS,
            &EVENTS,
        )
    }

    fn queue_scheduling_orchestrator_with_process<'a>(
        queue_repository: &'a dyn QueueRepository,
        history_repository: &'a dyn HistoryRepository,
        process_runner: &'a dyn TaskProcessRunner,
        diagnostics: &'a dyn Diagnostics,
        events: &'a dyn FrontendEventPublisher,
    ) -> QueueSchedulingPorts<'a> {
        static SETTINGS: StaticSettingsRepository = StaticSettingsRepository;
        static DIR_RESOLVER: StaticDownloadDirectoryResolver = StaticDownloadDirectoryResolver;
        static TERMINAL_OUTPUT: NoopTerminalOutputRepository = NoopTerminalOutputRepository;
        static SHUTDOWN_SCHEDULER: NoopShutdownScheduler = NoopShutdownScheduler;
        QueueSchedulingPorts::new(
            queue_repository,
            &SETTINGS,
            &DIR_RESOLVER,
            history_repository,
            &TERMINAL_OUTPUT,
            &SHUTDOWN_SCHEDULER,
            process_runner,
            diagnostics,
            events,
        )
    }

    #[derive(Default)]
    struct NoopFrontendEvents;

    impl FrontendEventPublisher for NoopFrontendEvents {
        fn task_error(&self, _task_id: &str, _message: &str) {}
        fn history_task_added(&self, _status: HistoryStatus, _task: &TaskSnapshot) {}
        fn queue_state_changed(&self) {}
        fn shutdown_countdown_cancelled(&self) {}
        fn shutdown_countdown_started(&self, _seconds: u64) {}
        fn task_progress(
            &self,
            _task_id: &str,
            _progress: Option<f32>,
            _speed: Option<&str>,
            _threads: Option<&str>,
        ) {
        }
        fn terminal_committed_line(&self, _task_id: &str, _line: &str) {}
        fn terminal_active_line(&self, _task_id: &str, _active_line: &str) {}
    }

    #[derive(Default)]
    struct NoopDiagnostics;

    impl Diagnostics for NoopDiagnostics {
        fn warn(&self, _message: &str) {}
    }

    struct NoopProcessRunner;

    impl TaskProcessRunner for NoopProcessRunner {
        fn start_task<'a>(
            &'a self,
            _request: TaskProcessStartRequest,
        ) -> ProcessRunnerFuture<'a, AppResult<()>> {
            Box::pin(async { Ok(()) })
        }

        fn shutdown_status<'a>(&'a self) -> ProcessRunnerFuture<'a, ProcessRunnerShutdownStatus> {
            Box::pin(async { ProcessRunnerShutdownStatus::Running })
        }
    }

    #[derive(Default)]
    struct CapturingDiagnostics {
        warnings: Mutex<Vec<String>>,
    }

    impl CapturingDiagnostics {
        fn warnings(&self) -> Vec<String> {
            self.warnings.lock().expect("diagnostic warnings").clone()
        }
    }

    impl Diagnostics for CapturingDiagnostics {
        fn warn(&self, message: &str) {
            self.warnings
                .lock()
                .expect("diagnostic warnings")
                .push(message.to_string());
        }
    }

    #[derive(Default)]
    struct CapturingFrontendEvents {
        queue_state_changed_count: Mutex<usize>,
        progress_task_ids: Mutex<Vec<String>>,
        terminal_committed_lines: Mutex<Vec<(String, String)>>,
        terminal_active_lines: Mutex<Vec<(String, String)>>,
        shutdown_countdown_started_seconds: Mutex<Vec<u64>>,
    }

    impl CapturingFrontendEvents {
        fn queue_state_changed_count(&self) -> usize {
            *self
                .queue_state_changed_count
                .lock()
                .expect("queue state events")
        }

        fn progress_count(&self) -> usize {
            self.progress_task_ids
                .lock()
                .expect("progress events")
                .len()
        }

        fn terminal_committed_count(&self) -> usize {
            self.terminal_committed_lines
                .lock()
                .expect("terminal committed events")
                .len()
        }

        fn terminal_active_lines(&self) -> Vec<(String, String)> {
            self.terminal_active_lines
                .lock()
                .expect("terminal active events")
                .clone()
        }

        fn shutdown_countdown_started_seconds(&self) -> Vec<u64> {
            self.shutdown_countdown_started_seconds
                .lock()
                .expect("shutdown countdown started events")
                .clone()
        }

        fn shutdown_countdown_started_count(&self) -> usize {
            self.shutdown_countdown_started_seconds
                .lock()
                .expect("shutdown countdown started events")
                .len()
        }
    }

    impl FrontendEventPublisher for CapturingFrontendEvents {
        fn task_error(&self, _task_id: &str, _message: &str) {}

        fn history_task_added(&self, _status: HistoryStatus, _task: &TaskSnapshot) {}

        fn queue_state_changed(&self) {
            *self
                .queue_state_changed_count
                .lock()
                .expect("queue state events") += 1;
        }

        fn shutdown_countdown_cancelled(&self) {}

        fn shutdown_countdown_started(&self, seconds: u64) {
            self.shutdown_countdown_started_seconds
                .lock()
                .expect("shutdown countdown started events")
                .push(seconds);
        }

        fn task_progress(
            &self,
            task_id: &str,
            _progress: Option<f32>,
            _speed: Option<&str>,
            _threads: Option<&str>,
        ) {
            self.progress_task_ids
                .lock()
                .expect("progress events")
                .push(task_id.to_string());
        }

        fn terminal_committed_line(&self, task_id: &str, line: &str) {
            self.terminal_committed_lines
                .lock()
                .expect("terminal committed events")
                .push((task_id.to_string(), line.to_string()));
        }

        fn terminal_active_line(&self, task_id: &str, active_line: &str) {
            self.terminal_active_lines
                .lock()
                .expect("terminal active events")
                .push((task_id.to_string(), active_line.to_string()));
        }
    }

    #[derive(Default)]
    struct FailingTerminalOutputRepository;

    impl TerminalOutputRepository for FailingTerminalOutputRepository {
        fn append_line(&self, _task_id: &str, _line: &str) -> AppResult<()> {
            Err(AppError::message("terminal output blocked"))
        }

        fn page(
            &self,
            _task_id: &str,
            _offset: usize,
            _limit: usize,
        ) -> AppResult<TerminalOutputPage> {
            Ok(TerminalOutputPage {
                lines: Vec::new(),
                offset: 0,
                total: 0,
                next_offset: 0,
                has_more_before: false,
                has_more_after: false,
            })
        }

        fn tail(&self, task_id: &str, limit: usize) -> AppResult<TerminalOutputPage> {
            self.page(task_id, 0, limit)
        }

        fn set_active_line(&self, _task_id: &str, _line: String) {}

        fn clear_active_line(&self, _task_id: &str) {}

        fn get_active_line(&self, _task_id: &str) -> TerminalActiveLine {
            TerminalActiveLine::Missing
        }
    }

    struct NoopTerminalOutputRepository;

    impl TerminalOutputRepository for NoopTerminalOutputRepository {
        fn append_line(&self, _task_id: &str, _line: &str) -> AppResult<()> {
            Ok(())
        }

        fn page(
            &self,
            _task_id: &str,
            _offset: usize,
            _limit: usize,
        ) -> AppResult<TerminalOutputPage> {
            Ok(TerminalOutputPage {
                lines: Vec::new(),
                offset: 0,
                total: 0,
                next_offset: 0,
                has_more_before: false,
                has_more_after: false,
            })
        }

        fn tail(&self, task_id: &str, limit: usize) -> AppResult<TerminalOutputPage> {
            self.page(task_id, 0, limit)
        }

        fn set_active_line(&self, _task_id: &str, _line: String) {}

        fn clear_active_line(&self, _task_id: &str) {}

        fn get_active_line(&self, _task_id: &str) -> TerminalActiveLine {
            TerminalActiveLine::Missing
        }
    }

    #[derive(Default)]
    struct SuccessfulProcessRunner {
        started_task_ids: Mutex<Vec<String>>,
    }

    impl SuccessfulProcessRunner {
        fn started_count(&self) -> usize {
            self.started_task_ids
                .lock()
                .expect("started task ids")
                .len()
        }
    }

    impl TaskProcessRunner for SuccessfulProcessRunner {
        fn start_task<'a>(
            &'a self,
            request: TaskProcessStartRequest,
        ) -> ProcessRunnerFuture<'a, AppResult<()>> {
            Box::pin(async move {
                self.started_task_ids
                    .lock()
                    .expect("started task ids")
                    .push(request.task_id);
                Ok(())
            })
        }

        fn shutdown_status<'a>(&'a self) -> ProcessRunnerFuture<'a, ProcessRunnerShutdownStatus> {
            Box::pin(async { ProcessRunnerShutdownStatus::Running })
        }
    }

    struct FailingProcessRunner {
        failures_before_success: usize,
        attempts: Mutex<usize>,
    }

    impl FailingProcessRunner {
        fn fail_once_then_succeed() -> Self {
            Self {
                failures_before_success: 1,
                attempts: Mutex::new(0),
            }
        }

        fn always_fail() -> Self {
            Self {
                failures_before_success: usize::MAX,
                attempts: Mutex::new(0),
            }
        }

        fn attempts(&self) -> usize {
            *self.attempts.lock().expect("start attempts")
        }
    }

    impl TaskProcessRunner for FailingProcessRunner {
        fn start_task<'a>(
            &'a self,
            request: TaskProcessStartRequest,
        ) -> ProcessRunnerFuture<'a, AppResult<()>> {
            Box::pin(async move {
                let mut attempts = self.attempts.lock().expect("start attempts");
                *attempts += 1;
                if *attempts <= self.failures_before_success {
                    Err(AppError::message(format!(
                        "failed to start {}",
                        request.task_id
                    )))
                } else {
                    Ok(())
                }
            })
        }

        fn shutdown_status<'a>(&'a self) -> ProcessRunnerFuture<'a, ProcessRunnerShutdownStatus> {
            Box::pin(async { ProcessRunnerShutdownStatus::Running })
        }
    }

    struct BlockingFailingProcessRunner {
        queue_path: PathBuf,
        blocked: Mutex<bool>,
    }

    impl BlockingFailingProcessRunner {
        fn new(queue_path: PathBuf) -> Self {
            Self {
                queue_path,
                blocked: Mutex::new(false),
            }
        }
    }

    impl TaskProcessRunner for BlockingFailingProcessRunner {
        fn start_task<'a>(
            &'a self,
            request: TaskProcessStartRequest,
        ) -> ProcessRunnerFuture<'a, AppResult<()>> {
            Box::pin(async move {
                let mut blocked = self.blocked.lock().expect("queue path blocked");
                if !*blocked {
                    if self.queue_path.is_dir() {
                        std::fs::remove_dir_all(&self.queue_path).map_err(|err| {
                            AppError::message(format!(
                                "unblock queue path before test block: {err}"
                            ))
                        })?;
                    } else if self.queue_path.exists() {
                        std::fs::remove_file(&self.queue_path).map_err(|err| {
                            AppError::message(format!("remove queue path before test block: {err}"))
                        })?;
                    }
                    std::fs::create_dir_all(&self.queue_path).map_err(|err| {
                        AppError::message(format!("block queue path for start failure: {err}"))
                    })?;
                    *blocked = true;
                }
                Err(AppError::message(format!(
                    "spawn failed for {}",
                    request.task_id
                )))
            })
        }

        fn shutdown_status<'a>(&'a self) -> ProcessRunnerFuture<'a, ProcessRunnerShutdownStatus> {
            Box::pin(async { ProcessRunnerShutdownStatus::Running })
        }
    }

    struct StaticSettingsRepository;

    impl SettingsRepository for StaticSettingsRepository {
        fn get(&self) -> AppSettings {
            AppSettings::default()
        }

        fn update(&self, settings: AppSettings) -> AppResult<AppSettings> {
            Ok(settings)
        }
    }

    struct AutoShutdownSettingsRepository;

    impl SettingsRepository for AutoShutdownSettingsRepository {
        fn get(&self) -> AppSettings {
            AppSettings {
                close_button_behavior: CloseButtonBehavior::CloseToTray,
                auto_action_on_complete: true,
                download_dir: None,
            }
        }

        fn update(&self, settings: AppSettings) -> AppResult<AppSettings> {
            Ok(settings)
        }
    }

    struct FailingShutdownScheduler;

    impl ShutdownScheduler for FailingShutdownScheduler {
        fn reset_for_new_run(&self) -> AppResult<ShutdownResetOutcome> {
            Ok(ShutdownResetOutcome::NoCountdown)
        }

        fn mark_run_failure(&self) {}

        fn clear_cancellation_after_reenable(&self) {}

        fn countdown_start_decision(&self) -> ShutdownCountdownStartDecision {
            ShutdownCountdownStartDecision::StartAllowed
        }

        fn start_countdown(&self) -> AppResult<u64> {
            Err(AppError::message("shutdown scheduler blocked"))
        }

        fn cancel_countdown(&self) -> AppResult<()> {
            Ok(())
        }
    }

    struct NoopShutdownScheduler;

    impl ShutdownScheduler for NoopShutdownScheduler {
        fn reset_for_new_run(&self) -> AppResult<ShutdownResetOutcome> {
            Ok(ShutdownResetOutcome::NoCountdown)
        }

        fn mark_run_failure(&self) {}

        fn clear_cancellation_after_reenable(&self) {}

        fn countdown_start_decision(&self) -> ShutdownCountdownStartDecision {
            ShutdownCountdownStartDecision::Blocked
        }

        fn start_countdown(&self) -> AppResult<u64> {
            Ok(0)
        }

        fn cancel_countdown(&self) -> AppResult<()> {
            Ok(())
        }
    }

    struct StaticDownloadDirectoryResolver;

    impl DownloadDirectoryResolver for StaticDownloadDirectoryResolver {
        fn resolve_download_dir(&self, _settings: &AppSettings) -> DownloadDirectory {
            DownloadDirectory::new("D:/Downloads")
        }
    }

    async fn add_queue_manager_task(
        queue_manager: &Arc<QueueManager>,
        payload: AddTaskPayload,
    ) -> (Task, bool) {
        let task = Task::new_queued(
            Uuid::new_v4().to_string(),
            payload.url,
            payload.save_name,
            payload.headers,
            chrono::Utc::now(),
        );
        let schedule_requested = queue_manager
            .add_task(task.clone())
            .await
            .expect("add task");
        (task, schedule_requested)
    }

    async fn prepare_task_for_terminal_failure(
        queue_manager: &Arc<QueueManager>,
        task_id: &str,
        error_message: &str,
    ) {
        assert!(matches!(
            queue_manager
                .prepare_task_failure(task_id, "first")
                .await
                .expect("prepare first failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("persist first retry")
            .expect("schedule first retry");
        assert!(matches!(
            queue_manager
                .prepare_task_failure(task_id, "second")
                .await
                .expect("prepare second failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("persist second retry")
            .expect("schedule second retry");
        assert!(matches!(
            queue_manager
                .prepare_task_failure(task_id, error_message)
                .await
                .expect("prepare terminal failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::Terminal)
        ));
    }

    #[tokio::test]
    async fn completed_run_with_auto_shutdown_enabled_starts_countdown() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let shutdown_manager = Arc::new(ShutdownManager::new());
        let settings_repository = AutoShutdownSettingsRepository;
        let download_directory_resolver = StaticDownloadDirectoryResolver;
        let process_runner = SuccessfulProcessRunner::default();
        let diagnostics = NoopDiagnostics;
        let events = CapturingFrontendEvents::default();
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        let scheduling_ports = QueueSchedulingPorts::new(
            queue_manager.as_ref(),
            &settings_repository,
            &download_directory_resolver,
            history_store.as_ref(),
            &NoopTerminalOutputRepository,
            shutdown_manager.as_ref(),
            &process_runner,
            &diagnostics,
            &events,
        );
        scheduling_ports
            .handle_completed_child_exit(&task.id, "D:/Videos/test.mp4")
            .await;

        assert_eq!(
            events.shutdown_countdown_started_seconds(),
            vec![crate::adapters::shutdown::shutdown_seconds()]
        );

        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn handle_queue_pause_leaves_current_download_running() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let task_runner = Arc::new(TaskRunner::new());
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;
        let child = spawn_sleeping_child().await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");
        task_runner
            .insert_running_task_for_test(task.id.clone(), child)
            .await;
        task_runner.begin_wait_for_test(&task.id).await;

        let events = CapturingFrontendEvents::default();
        let ports = QueueMutationPorts::new(queue_manager.as_ref(), &events);
        ports
            .handle_queue_pause()
            .await
            .expect("pause queue succeeds");

        let state = queue_manager.get_state().await;
        let active_task = state
            .tasks()
            .iter()
            .find(|t| t.id == task.id)
            .expect("task exists");

        assert!(!state.is_running());
        assert_eq!(state.current_task_id(), Some(task.id.as_str()));
        assert_eq!(active_task.status, TaskStatus::Downloading);
        assert!(task_runner.is_task_running(&task.id).await);
        assert_eq!(events.queue_state_changed_count(), 1);
    }

    #[tokio::test]
    async fn task_output_progress_events_publish_only_after_live_state_update() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let cli_output_path = std::env::temp_dir().join(format!("cli-output-{}", Uuid::new_v4()));
        let cli_output_store = CliOutputStore::new(cli_output_path.clone());
        let diagnostics = NoopDiagnostics;
        let events = CapturingFrontendEvents::default();
        let output_ports = TaskOutputEventPorts::new(
            queue_manager.as_ref(),
            &cli_output_store,
            &diagnostics,
            &events,
        );

        output_ports
            .handle_task_output_event(TaskOutputEvent::Progress {
                id: "missing-task".to_string(),
                progress: Some(0.5),
                speed: Some("1 MB/s".to_string()),
                threads: None,
            })
            .await;
        assert_eq!(events.progress_count(), 0);

        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        output_ports
            .handle_task_output_event(TaskOutputEvent::Progress {
                id: task.id.clone(),
                progress: Some(0.5),
                speed: Some("1 MB/s".to_string()),
                threads: Some("4".to_string()),
            })
            .await;

        let state = queue_manager.get_state().await;
        let _updated = state
            .tasks()
            .iter()
            .find(|candidate| candidate.id == task.id)
            .expect("task exists");
        assert_eq!(events.progress_count(), 1);

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(cli_output_path);
    }

    #[tokio::test]
    async fn terminal_committed_lines_publish_only_after_persistence() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let terminal_output_repository = FailingTerminalOutputRepository;
        let diagnostics = NoopDiagnostics;
        let events = CapturingFrontendEvents::default();
        let output_ports = TaskOutputEventPorts::new(
            queue_manager.as_ref(),
            &terminal_output_repository,
            &diagnostics,
            &events,
        );

        output_ports
            .handle_task_output_event(TaskOutputEvent::TerminalCommittedLine {
                id: "task-1".to_string(),
                line: "persist me first".to_string(),
            })
            .await;

        assert_eq!(events.terminal_committed_count(), 0);

        let _ = std::fs::remove_file(queue_path);
    }

    #[tokio::test]
    async fn terminal_active_lines_publish_after_repository_sync() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let cli_output_path = std::env::temp_dir().join(format!("cli-output-{}", Uuid::new_v4()));
        let cli_output_store = CliOutputStore::new(cli_output_path.clone());
        let diagnostics = NoopDiagnostics;
        let events = CapturingFrontendEvents::default();
        let output_ports = TaskOutputEventPorts::new(
            queue_manager.as_ref(),
            &cli_output_store,
            &diagnostics,
            &events,
        );

        output_ports
            .handle_task_output_event(TaskOutputEvent::TerminalActiveLine {
                id: "task-1".to_string(),
                active_line: "Progress: 50%".to_string(),
            })
            .await;

        assert!(matches!(
            cli_output_store.get_active_line("task-1"),
            TerminalActiveLine::Present(line) if line == "Progress: 50%"
        ));
        assert_eq!(
            events.terminal_active_lines(),
            vec![("task-1".to_string(), "Progress: 50%".to_string())]
        );

        output_ports
            .handle_task_output_event(TaskOutputEvent::TerminalActiveLine {
                id: "task-1".to_string(),
                active_line: String::new(),
            })
            .await;

        assert!(matches!(
            cli_output_store.get_active_line("task-1"),
            TerminalActiveLine::Missing
        ));
        assert_eq!(
            events.terminal_active_lines(),
            vec![
                ("task-1".to_string(), "Progress: 50%".to_string()),
                ("task-1".to_string(), String::new()),
            ]
        );

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(cli_output_path);
    }

    #[tokio::test]
    async fn child_exit_publishes_queue_state_after_scheduling_next_task() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let settings_repository = AutoShutdownSettingsRepository;
        let download_directory_resolver = StaticDownloadDirectoryResolver;
        let process_runner = SuccessfulProcessRunner::default();
        let shutdown_manager = ShutdownManager::new();
        let diagnostics = NoopDiagnostics;
        let events = CapturingFrontendEvents::default();
        let completed_payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (completed_task, _) = add_queue_manager_task(&queue_manager, completed_payload).await;
        let next_payload = AddTaskPayload {
            url: "https://example.com/next.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (next_task, _) = add_queue_manager_task(&queue_manager, next_payload).await;
        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        let scheduling_ports = QueueSchedulingPorts::new(
            queue_manager.as_ref(),
            &settings_repository,
            &download_directory_resolver,
            history_store.as_ref(),
            &NoopTerminalOutputRepository,
            &shutdown_manager,
            &process_runner,
            &diagnostics,
            &events,
        );

        scheduling_ports
            .handle_completed_child_exit(&completed_task.id, "D:/Videos/test.mp4")
            .await;

        let state = queue_manager.get_state().await;
        let scheduled = state
            .tasks()
            .iter()
            .find(|candidate| candidate.id == next_task.id)
            .expect("task remains in queue");
        assert_eq!(scheduled.status, TaskStatus::Downloading);
        assert_eq!(state.current_task_id(), Some(next_task.id.as_str()));
        assert_eq!(process_runner.started_count(), 1);
        assert_eq!(events.queue_state_changed_count(), 2);
        assert_eq!(
            events.shutdown_countdown_started_count(),
            0,
            "no shutdown countdown while task is scheduled/downloading"
        );

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn child_exit_reports_shutdown_countdown_start_failure() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let settings_repository = AutoShutdownSettingsRepository;
        let download_directory_resolver = StaticDownloadDirectoryResolver;
        let process_runner = SuccessfulProcessRunner::default();
        let shutdown_scheduler = FailingShutdownScheduler;
        let diagnostics = CapturingDiagnostics::default();
        let events = CapturingFrontendEvents::default();
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        let scheduling_ports = QueueSchedulingPorts::new(
            queue_manager.as_ref(),
            &settings_repository,
            &download_directory_resolver,
            history_store.as_ref(),
            &NoopTerminalOutputRepository,
            &shutdown_scheduler,
            &process_runner,
            &diagnostics,
            &events,
        );

        scheduling_ports
            .handle_completed_child_exit(&task.id, "D:/Videos/test.mp4")
            .await;

        assert!(
            diagnostics
                .warnings()
                .iter()
                .any(|warning| warning
                    .contains("Failed to start shutdown countdown after completion"))
        );

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn task_completion_publishes_active_line_clear() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let cli_output_path = std::env::temp_dir().join(format!("cli-output-{}", Uuid::new_v4()));
        let cli_output_store = CliOutputStore::new(cli_output_path.clone());
        let settings_repository = StaticSettingsRepository;
        let download_directory_resolver = StaticDownloadDirectoryResolver;
        let process_runner = SuccessfulProcessRunner::default();
        let shutdown_manager = ShutdownManager::new();
        let diagnostics = NoopDiagnostics;
        let events = CapturingFrontendEvents::default();
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");
        cli_output_store.set_active_line(&task.id, "Progress: 50%".to_string());

        let scheduling_ports = QueueSchedulingPorts::new(
            queue_manager.as_ref(),
            &settings_repository,
            &download_directory_resolver,
            history_store.as_ref(),
            &cli_output_store,
            &shutdown_manager,
            &process_runner,
            &diagnostics,
            &events,
        );
        let lifecycle_ports = TaskLifecyclePorts::new(scheduling_ports);

        lifecycle_ports
            .handle_completed_child_exit(&task.id, "D:/Videos/test.mp4")
            .await;

        assert!(matches!(
            cli_output_store.get_active_line(&task.id),
            TerminalActiveLine::Missing
        ));
        assert_eq!(
            events.terminal_active_lines(),
            vec![(task.id.clone(), String::new())]
        );

        // 回归保护：完成任务的历史记录必须持久化真实下载路径，
        // 而不是 None（曾因 snapshot 生成顺序导致 outputPath 丢失）。
        let completed_page = history_store
            .get_page(crate::domain::history::HistoryStatus::Completed, 0, 10)
            .expect("history page");
        let completed_task = completed_page
            .tasks
            .iter()
            .find(|t| t.id == task.id)
            .expect("completed task in history");
        assert_eq!(
            completed_task.output_path.as_deref(),
            Some("D:/Videos/test.mp4"),
            "completed history task must persist output_path"
        );

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
        let _ = std::fs::remove_dir_all(cli_output_path);
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
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");
        assert!(matches!(
            queue_manager
                .prepare_task_failure(&task.id, "first")
                .await
                .expect("prepare first failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("persist rescheduled task")
            .expect("rescheduled task");
        assert!(matches!(
            queue_manager
                .prepare_task_failure(&task.id, "second")
                .await
                .expect("prepare second failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));

        let process_runner = FailingProcessRunner::always_fail();
        let diagnostics = NoopDiagnostics;
        let events = NoopFrontendEvents;
        let scheduling_ports = queue_scheduling_orchestrator_with_process(
            queue_manager.as_ref(),
            history_store.as_ref(),
            &process_runner,
            &diagnostics,
            &events,
        );
        scheduling_ports
            .handle_queue_start()
            .await
            .expect("drive start failure through queue-start scheduling intent");

        let state = queue_manager.get_state().await;
        assert!(state.tasks().is_empty());
        let page = history_store
            .get_page(HistoryStatus::Failed, 0, 20)
            .expect("history page");
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.tasks[0].id, task.id);
        assert_eq!(process_runner.attempts(), 1);

        std::fs::remove_dir_all(history_path).expect("cleanup history");
    }

    #[tokio::test]
    async fn handle_start_failure_reports_retry_scheduled() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");

        let process_runner = FailingProcessRunner::fail_once_then_succeed();
        let diagnostics = NoopDiagnostics;
        let events = NoopFrontendEvents;
        let scheduling_ports = queue_scheduling_orchestrator_with_process(
            queue_manager.as_ref(),
            history_store.as_ref(),
            &process_runner,
            &diagnostics,
            &events,
        );
        scheduling_ports
            .handle_queue_start()
            .await
            .expect("retry then start task through queue-start scheduling intent");

        let state = queue_manager.get_state().await;
        let retried = state
            .tasks()
            .iter()
            .find(|candidate| candidate.id == task.id)
            .expect("task remains in queue");
        assert_eq!(retried.status, TaskStatus::Downloading);
        assert_eq!(retried.retry_count, 1);
        assert_eq!(state.current_task_id(), Some(task.id.as_str()));
        assert_eq!(process_runner.attempts(), 2);

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn handle_start_failure_finalizes_task_when_history_append_fails() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        std::fs::write(&history_path, b"blocked").expect("create blocking file");
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");
        assert!(matches!(
            queue_manager
                .prepare_task_failure(&task.id, "first")
                .await
                .expect("prepare first failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("persist rescheduled task")
            .expect("rescheduled task");
        assert!(matches!(
            queue_manager
                .prepare_task_failure(&task.id, "second")
                .await
                .expect("prepare second failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));

        let process_runner = FailingProcessRunner::always_fail();
        let diagnostics = NoopDiagnostics;
        let events = NoopFrontendEvents;
        let scheduling_ports = queue_scheduling_orchestrator_with_process(
            queue_manager.as_ref(),
            history_store.as_ref(),
            &process_runner,
            &diagnostics,
            &events,
        );
        scheduling_ports
            .handle_queue_start()
            .await
            .expect("terminal start failure is finalized even when history append fails");

        let state = queue_manager.get_state().await;
        assert!(state.tasks().iter().all(|t| t.id != task.id));
        assert!(state.current_task_id().is_none());
        assert_eq!(state.pending_history_tasks().len(), 1);
        assert_eq!(state.pending_history_tasks()[0].id, task.id);
        assert_eq!(state.pending_history_tasks()[0].status, TaskStatus::Failed);
        assert_eq!(process_runner.attempts(), 1);

        let _ = std::fs::remove_file(history_path);
    }

    #[tokio::test]
    async fn pending_completed_history_task_flushes_after_restart() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        std::fs::write(&history_path, b"blocked").expect("create blocking file");
        let blocked_history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        let terminal_ports =
            terminal_history_orchestrator(queue_manager.as_ref(), blocked_history_store.as_ref());
        let result =
            handle_completed_task_history(&terminal_ports, &task.id, "D:/Videos/test.mp4").await;
        assert!(result.is_err());

        std::fs::remove_file(&history_path).expect("unblock history path");
        let recovered_queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));

        let terminal_ports =
            terminal_history_orchestrator(recovered_queue_manager.as_ref(), history_store.as_ref());
        let flushed = flush_pending_history_tasks(&terminal_ports).await;

        assert_eq!(flushed.expect("flush pending history").len(), 1);
        let page = history_store
            .get_page(HistoryStatus::Completed, 0, 20)
            .expect("history page");
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.tasks[0].id, task.id);
        assert!(recovered_queue_manager
            .get_state()
            .await
            .pending_history_tasks()
            .is_empty());

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn pending_history_flush_recovers_after_clear_persistence_failure_without_duplicates() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        std::fs::write(&history_path, b"blocked").expect("create blocking file");
        let blocked_history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        let terminal_ports =
            terminal_history_orchestrator(queue_manager.as_ref(), blocked_history_store.as_ref());
        let result =
            handle_completed_task_history(&terminal_ports, &task.id, "D:/Videos/test.mp4").await;
        assert!(result.is_err());

        std::fs::remove_file(&history_path).expect("unblock history path");
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));

        std::fs::remove_file(&queue_path).expect("remove persisted queue file");
        std::fs::create_dir_all(&queue_path).expect("block queue persistence path");

        let terminal_ports =
            terminal_history_orchestrator(queue_manager.as_ref(), history_store.as_ref());
        let result = flush_pending_history_tasks(&terminal_ports).await;
        assert!(result.is_err());
        let page = history_store
            .get_page(HistoryStatus::Completed, 0, 20)
            .expect("history page after failed clear");
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.tasks[0].id, task.id);
        assert_eq!(
            queue_manager
                .get_state()
                .await
                .pending_history_tasks()
                .len(),
            1
        );

        std::fs::remove_dir_all(&queue_path).expect("unblock queue persistence path");
        let flushed = flush_pending_history_tasks(&terminal_ports)
            .await
            .expect("retry pending history flush");
        assert_eq!(flushed.len(), 1);

        let page = history_store
            .get_page(HistoryStatus::Completed, 0, 20)
            .expect("history page after retry");
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.tasks[0].id, task.id);
        assert!(queue_manager
            .get_state()
            .await
            .pending_history_tasks()
            .is_empty());

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn handle_start_failure_clears_dead_active_task_when_failure_persistence_fails() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        let process_runner = BlockingFailingProcessRunner::new(queue_path.clone());
        let diagnostics = NoopDiagnostics;
        let events = NoopFrontendEvents;
        let scheduling_ports = queue_scheduling_orchestrator_with_process(
            queue_manager.as_ref(),
            history_store.as_ref(),
            &process_runner,
            &diagnostics,
            &events,
        );
        scheduling_ports
            .handle_queue_start()
            .await
            .expect("queue-start scheduling intent handles start-failure persistence error");

        let state = queue_manager.get_state().await;
        let failed = state
            .tasks()
            .iter()
            .find(|candidate| candidate.id == task.id)
            .expect("task remains visible for recovery");
        assert!(!state.is_running());
        assert!(state.current_task_id().is_none());
        assert_eq!(failed.status, TaskStatus::Failed);
        let expected_error = format!("spawn failed for {}", task.id);
        assert_eq!(
            failed.error_message.as_deref(),
            Some(expected_error.as_str())
        );

        let _ = std::fs::remove_dir_all(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn exited_child_failure_clears_dead_active_task_when_failure_persistence_fails() {
        let queue_path = temp_persistence_path();
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        std::fs::remove_file(&queue_path).expect("remove persisted queue file");
        std::fs::create_dir_all(&queue_path).expect("block queue persistence path");

        let scheduling_ports =
            queue_scheduling_orchestrator(queue_manager.as_ref(), history_store.as_ref());
        scheduling_ports
            .handle_failed_child_exit(&task.id, "child process failed")
            .await;

        let state = queue_manager.get_state().await;
        let failed = state
            .tasks()
            .iter()
            .find(|candidate| candidate.id == task.id)
            .expect("task remains visible for recovery");
        assert!(!state.is_running());
        assert!(state.current_task_id().is_none());
        assert_eq!(failed.status, TaskStatus::Failed);
        assert_eq!(
            failed.error_message.as_deref(),
            Some("child process failed")
        );

        let _ = std::fs::remove_dir_all(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn exited_child_failure_reports_retry_transition() {
        let queue_path = temp_persistence_path();
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        let scheduling_ports =
            queue_scheduling_orchestrator(queue_manager.as_ref(), history_store.as_ref());
        scheduling_ports
            .handle_failed_child_exit(&task.id, "retryable failure")
            .await;

        let state = queue_manager.get_state().await;
        let current = state.current_task_id();
        assert!(
            current == Some(task.id.as_str()),
            "retry-scheduled task should be scheduled again after failed child-exit continuation"
        );
        let found = state
            .tasks()
            .iter()
            .find(|t| t.id == task.id && t.status == TaskStatus::Downloading);
        assert!(
            found.is_some(),
            "task should be downloading again after failed child-exit continuation"
        );
        assert_eq!(
            found
                .expect("task should have status Downloading")
                .retry_count,
            1,
            "retry_count should be 1 after retry failure"
        );

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn terminal_failed_child_exit_marks_run_failure_for_shutdown_countdown() {
        let queue_path = temp_persistence_path();
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");
        assert!(matches!(
            queue_manager
                .prepare_task_failure(&task.id, "first retryable failure")
                .await
                .expect("prepare first retryable failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("persist first retry")
            .expect("schedule first retry");
        assert!(matches!(
            queue_manager
                .prepare_task_failure(&task.id, "second retryable failure")
                .await
                .expect("prepare second retryable failure"),
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        ));
        queue_manager
            .schedule_next()
            .await
            .expect("persist second retry")
            .expect("schedule second retry");

        let settings_repository = AutoShutdownSettingsRepository;
        let download_directory_resolver = StaticDownloadDirectoryResolver;
        let terminal_output_repository = NoopTerminalOutputRepository;
        let shutdown_manager = ShutdownManager::new();
        let process_runner = NoopProcessRunner;
        let diagnostics = NoopDiagnostics;
        let events = NoopFrontendEvents;
        let scheduling_ports = QueueSchedulingPorts::new(
            queue_manager.as_ref(),
            &settings_repository,
            &download_directory_resolver,
            history_store.as_ref(),
            &terminal_output_repository,
            &shutdown_manager,
            &process_runner,
            &diagnostics,
            &events,
        );

        assert!(matches!(
            shutdown_manager.countdown_start_decision(),
            ShutdownCountdownStartDecision::StartAllowed
        ));
        scheduling_ports
            .handle_failed_child_exit(&task.id, "terminal failure")
            .await;

        assert!(matches!(
            shutdown_manager.countdown_start_decision(),
            ShutdownCountdownStartDecision::Blocked
        ));

        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn completed_task_finalizes_queue_when_history_append_fails() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        std::fs::write(&history_path, b"blocked").expect("create blocking file");
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        let terminal_ports =
            terminal_history_orchestrator(queue_manager.as_ref(), history_store.as_ref());
        let result =
            handle_completed_task_history(&terminal_ports, &task.id, "D:/Videos/test.mp4").await;
        assert!(result.is_err());

        let state = queue_manager.get_state().await;
        assert!(state.tasks().iter().all(|t| t.id != task.id));
        assert!(state.current_task_id().is_none());
        assert_eq!(state.pending_history_tasks().len(), 1);
        assert_eq!(state.pending_history_tasks()[0].id, task.id);
        assert_eq!(
            state.pending_history_tasks()[0].status,
            TaskStatus::Completed
        );

        let _ = std::fs::remove_file(history_path);
    }

    #[tokio::test]
    async fn completed_task_does_not_append_history_when_queue_finalize_fails() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        std::fs::remove_file(&queue_path).expect("remove persisted queue file");
        std::fs::create_dir_all(&queue_path).expect("block queue persistence path");

        let terminal_ports =
            terminal_history_orchestrator(queue_manager.as_ref(), history_store.as_ref());
        let result =
            handle_completed_task_history(&terminal_ports, &task.id, "D:/Videos/test.mp4").await;

        assert!(result.is_err());
        let page = history_store
            .get_page(HistoryStatus::Completed, 0, 20)
            .expect("history page");
        assert!(page.tasks.is_empty());

        let _ = std::fs::remove_dir_all(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn terminal_failure_finalizes_queue_when_history_append_fails() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-file-{}", Uuid::new_v4()));
        std::fs::write(&history_path, b"blocked").expect("create blocking file");
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        prepare_task_for_terminal_failure(&queue_manager, &task.id, "terminal failure").await;

        let terminal_ports =
            terminal_history_orchestrator(queue_manager.as_ref(), history_store.as_ref());
        let result = handle_terminal_failure_task_history(&terminal_ports, &task.id).await;
        assert!(result.is_err());

        let state = queue_manager.get_state().await;
        assert!(state.tasks().iter().all(|t| t.id != task.id));
        assert!(state.current_task_id().is_none());
        assert_eq!(state.pending_history_tasks().len(), 1);
        assert_eq!(state.pending_history_tasks()[0].id, task.id);
        assert_eq!(state.pending_history_tasks()[0].status, TaskStatus::Failed);

        let _ = std::fs::remove_file(history_path);
    }

    #[tokio::test]
    async fn terminal_failure_does_not_append_history_when_queue_finalize_fails() {
        let queue_path = temp_persistence_path();
        let queue_manager = Arc::new(QueueManager::new(queue_path.clone()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: None,
            headers: None,
        };
        let (task, _) = add_queue_manager_task(&queue_manager, payload).await;

        queue_manager
            .set_run_status(QueueRunStatus::Running)
            .await
            .expect("set running");
        queue_manager
            .schedule_next()
            .await
            .expect("persist scheduled task")
            .expect("scheduled task");

        prepare_task_for_terminal_failure(&queue_manager, &task.id, "terminal failure").await;

        std::fs::remove_file(&queue_path).expect("remove persisted queue file");
        std::fs::create_dir_all(&queue_path).expect("block queue persistence path");

        let terminal_ports =
            terminal_history_orchestrator(queue_manager.as_ref(), history_store.as_ref());
        let result = handle_terminal_failure_task_history(&terminal_ports, &task.id).await;

        assert!(result.is_err());
        let page = history_store
            .get_page(HistoryStatus::Failed, 0, 20)
            .expect("history page");
        assert!(page.tasks.is_empty());

        let _ = std::fs::remove_dir_all(queue_path);
        let _ = std::fs::remove_dir_all(history_path);
    }

    #[tokio::test]
    async fn terminal_failure_does_not_append_stale_task_without_queue_record() {
        let queue_manager = Arc::new(QueueManager::new(temp_persistence_path()));
        let history_path = std::env::temp_dir().join(format!("history-{}", Uuid::new_v4()));
        let history_store = Arc::new(HistoryStore::new(history_path.clone()));
        let stale_task_id = Uuid::new_v4().to_string();

        let terminal_ports =
            terminal_history_orchestrator(queue_manager.as_ref(), history_store.as_ref());
        let result = handle_terminal_failure_task_history(&terminal_ports, &stale_task_id)
            .await
            .expect("stale terminal record should be ignored");

        assert!(matches!(result, TerminalHistoryRecordOutcome::Ignored));
        let page = history_store
            .get_page(HistoryStatus::Failed, 0, 20)
            .expect("history page");
        assert!(page.tasks.is_empty());

        let _ = std::fs::remove_dir_all(history_path);
    }
}
