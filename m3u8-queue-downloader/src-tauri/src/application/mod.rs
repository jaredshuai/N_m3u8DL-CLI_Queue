pub(crate) mod app_error;
pub(crate) mod close_policy;
pub(crate) mod download_directory;
pub(crate) mod download_directory_orchestrator;
pub(crate) mod exit_orchestrator;
pub(crate) mod exit_use_cases;
pub(crate) mod history_orchestrator;
pub(crate) mod history_repository_outcomes;
pub(crate) mod history_task_page;
pub(crate) mod history_use_cases;
pub(crate) mod process_runner_outcomes;
pub(crate) mod query_models;
pub(crate) mod queue_mutation_orchestrator;
pub(crate) mod queue_query_orchestrator;
pub(crate) mod queue_repository_outcomes;
pub(crate) mod queue_requests;
pub(crate) mod queue_scheduler_outcomes;
pub(crate) mod queue_scheduling_orchestrator;
pub(crate) mod queue_state_snapshot;
pub(crate) mod run_completion_orchestrator;
pub(crate) mod settings;
pub(crate) mod settings_orchestrator;
pub(crate) mod settings_query_orchestrator;
pub(crate) mod shutdown_scheduler_outcomes;
pub(crate) mod task_creation_orchestrator;
pub(crate) mod task_output_event_orchestrator;
pub(crate) mod task_process_events;
pub(crate) mod task_process_start_request;
pub(crate) mod task_runtime_state;
pub(crate) mod task_snapshot;
pub(crate) mod terminal_history_orchestrator;
pub(crate) mod terminal_history_use_cases;
pub(crate) mod terminal_output_orchestrator;
pub(crate) mod terminal_output_outcomes;
pub(crate) mod terminal_output_page;
pub(crate) mod terminal_output_use_cases;

// Port trait re-exports: port traits are defined by the application layer
// (the contract it requires from adapters), so they belong here.
// The physical files remain in src/ports/ for now; these re-exports
// allow callers to use `crate::application::Clock` instead of
// `crate::ports::Clock`, establishing the correct dependency direction.
pub(crate) use crate::ports::application_control::ApplicationControl;
pub(crate) use crate::ports::clock::Clock;
pub(crate) use crate::ports::diagnostics::Diagnostics;
pub(crate) use crate::ports::directory_opener::DirectoryOpener;
pub(crate) use crate::ports::download_directory_resolver::DownloadDirectoryResolver;
pub(crate) use crate::ports::event_publisher::FrontendEventPublisher;
pub(crate) use crate::ports::history_repository::HistoryRepository;
pub(crate) use crate::ports::process_runner::{TaskProcessRunner, TaskProcessSupervisor};
pub(crate) use crate::ports::queue_repository::QueueRepository;
pub(crate) use crate::ports::settings_repository::SettingsRepository;
pub(crate) use crate::ports::shutdown_scheduler::ShutdownScheduler;
pub(crate) use crate::ports::task_id_generator::TaskIdGenerator;
pub(crate) use crate::ports::terminal_output_repository::TerminalOutputRepository;
