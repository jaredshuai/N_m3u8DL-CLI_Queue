use crate::application::download_directory_orchestrator::DownloadDirectoryPorts;
use crate::application::exit_orchestrator::ExitPorts;
use crate::application::history_orchestrator::HistoryPorts;
use crate::application::queue_mutation_orchestrator::QueueMutationPorts;
use crate::application::queue_query_orchestrator::QueueQueryPorts;
use crate::application::queue_scheduling_orchestrator::QueueSchedulingPorts;
use crate::application::settings_orchestrator::SettingsPorts;
use crate::application::settings_query_orchestrator::SettingsQueryPorts;
use crate::application::task_creation_orchestrator::TaskCreationPorts;
use crate::application::task_output_event_orchestrator::TaskOutputEventPorts;
use crate::application::terminal_history_orchestrator::TerminalHistoryPorts;
use crate::application::terminal_output_orchestrator::TerminalOutputPorts;
use crate::ports::application_control::ApplicationControl;
use crate::ports::artifact_inventory::ArtifactInventory;
use crate::ports::clock::Clock;
use crate::ports::diagnostics::Diagnostics;
use crate::ports::directory_opener::DirectoryOpener;
use crate::ports::download_directory_resolver::DownloadDirectoryResolver;
use crate::ports::event_publisher::FrontendEventPublisher;
use crate::ports::history_repository::HistoryRepository;
use crate::ports::process_runner::{
    TaskProcessRunner, TaskProcessRunnerFactory, TaskProcessSupervisor,
};
use crate::ports::queue_repository::QueueRepository;
use crate::ports::settings_repository::SettingsRepository;
use crate::ports::shutdown_scheduler::ShutdownScheduler;
use crate::ports::task_id_generator::TaskIdGenerator;
use crate::ports::terminal_output_repository::TerminalOutputRepository;
use std::sync::Arc;

#[derive(Clone)]
pub struct DependencyGraph {
    pub(in crate::composition) terminal_output_repository: Arc<dyn TerminalOutputRepository>,
    pub(in crate::composition) history_repository: Arc<dyn HistoryRepository>,
    pub(in crate::composition) queue_repository: Arc<dyn QueueRepository>,
    pub(in crate::composition) settings_repository: Arc<dyn SettingsRepository>,
    pub(in crate::composition) download_directory_resolver: Arc<dyn DownloadDirectoryResolver>,
    pub(in crate::composition) directory_opener: Arc<dyn DirectoryOpener>,
    pub(in crate::composition) application_control: Arc<dyn ApplicationControl>,
    pub(in crate::composition) shutdown_scheduler: Arc<dyn ShutdownScheduler>,
    pub(in crate::composition) task_process_supervisor: Arc<dyn TaskProcessSupervisor>,
    pub(in crate::composition) task_process_runner_factory: Arc<dyn TaskProcessRunnerFactory>,
    pub(in crate::composition) task_id_generator: Arc<dyn TaskIdGenerator>,
    pub(in crate::composition) clock: Arc<dyn Clock>,
    pub(in crate::composition) artifact_inventory: Arc<dyn ArtifactInventory>,
    pub(in crate::composition) diagnostics: Arc<dyn Diagnostics>,
}

impl DependencyGraph {
    pub fn new(
        terminal_output_repository: Arc<dyn TerminalOutputRepository>,
        history_repository: Arc<dyn HistoryRepository>,
        queue_repository: Arc<dyn QueueRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
        download_directory_resolver: Arc<dyn DownloadDirectoryResolver>,
        directory_opener: Arc<dyn DirectoryOpener>,
        application_control: Arc<dyn ApplicationControl>,
        shutdown_scheduler: Arc<dyn ShutdownScheduler>,
        task_process_supervisor: Arc<dyn TaskProcessSupervisor>,
        task_process_runner_factory: Arc<dyn TaskProcessRunnerFactory>,
        task_id_generator: Arc<dyn TaskIdGenerator>,
        clock: Arc<dyn Clock>,
        artifact_inventory: Arc<dyn ArtifactInventory>,
        diagnostics: Arc<dyn Diagnostics>,
    ) -> Self {
        Self {
            terminal_output_repository,
            history_repository,
            queue_repository,
            settings_repository,
            download_directory_resolver,
            directory_opener,
            application_control,
            shutdown_scheduler,
            task_process_supervisor,
            task_process_runner_factory,
            task_id_generator,
            clock,
            artifact_inventory,
            diagnostics,
        }
    }

    pub(in crate::composition) fn queue_scheduling_orchestrator<'a>(
        &'a self,
        events: &'a dyn FrontendEventPublisher,
        process_runner: &'a dyn TaskProcessRunner,
    ) -> QueueSchedulingPorts<'a> {
        QueueSchedulingPorts::new(
            self.queue_repository.as_ref(),
            self.settings_repository.as_ref(),
            self.download_directory_resolver.as_ref(),
            self.history_repository.as_ref(),
            self.terminal_output_repository.as_ref(),
            self.shutdown_scheduler.as_ref(),
            process_runner,
            self.artifact_inventory.as_ref(),
            self.clock.as_ref(),
            self.diagnostics.as_ref(),
            events,
        )
    }

    pub(in crate::composition) fn queue_mutation_orchestrator<'a>(
        &'a self,
        events: &'a dyn FrontendEventPublisher,
    ) -> QueueMutationPorts<'a> {
        QueueMutationPorts::new(self.queue_repository.as_ref(), events)
    }

    pub(in crate::composition) fn queue_query_orchestrator(&self) -> QueueQueryPorts<'_> {
        QueueQueryPorts::new(self.queue_repository.as_ref())
    }

    pub(in crate::composition) fn create_task_process_runner(&self) -> Arc<dyn TaskProcessRunner> {
        self.task_process_runner_factory.create_process_runner()
    }

    pub(in crate::composition) fn task_creation_orchestrator(&self) -> TaskCreationPorts<'_> {
        TaskCreationPorts::new(self.task_id_generator.as_ref(), self.clock.as_ref())
    }

    pub(in crate::composition) fn terminal_history_orchestrator(&self) -> TerminalHistoryPorts<'_> {
        TerminalHistoryPorts::new(
            self.queue_repository.as_ref(),
            self.history_repository.as_ref(),
        )
    }

    pub(in crate::composition) fn history_orchestrator(&self) -> HistoryPorts {
        HistoryPorts::new(self.history_repository.clone())
    }

    pub(in crate::composition) fn task_output_event_orchestrator<'a>(
        &'a self,
        events: &'a dyn FrontendEventPublisher,
    ) -> TaskOutputEventPorts<'a> {
        TaskOutputEventPorts::new(
            self.queue_repository.as_ref(),
            self.terminal_output_repository.as_ref(),
            self.diagnostics.as_ref(),
            events,
        )
    }

    pub(in crate::composition) fn terminal_output_orchestrator(&self) -> TerminalOutputPorts {
        TerminalOutputPorts::new(self.terminal_output_repository.clone())
    }

    pub(in crate::composition) fn settings_orchestrator<'a>(
        &'a self,
        events: &'a dyn FrontendEventPublisher,
    ) -> SettingsPorts<'a> {
        SettingsPorts::new(
            self.settings_repository.as_ref(),
            self.shutdown_scheduler.as_ref(),
            events,
        )
    }

    pub(in crate::composition) fn settings_query_orchestrator(&self) -> SettingsQueryPorts<'_> {
        SettingsQueryPorts::new(self.settings_repository.as_ref())
    }

    pub(in crate::composition) fn download_directory_orchestrator(
        &self,
    ) -> DownloadDirectoryPorts<'_> {
        DownloadDirectoryPorts::new(
            self.settings_repository.as_ref(),
            self.download_directory_resolver.as_ref(),
            self.directory_opener.as_ref(),
        )
    }

    pub(in crate::composition) fn exit_orchestrator<'a>(
        &'a self,
        events: &'a dyn FrontendEventPublisher,
    ) -> ExitPorts<'a> {
        ExitPorts::new(
            self.settings_repository.as_ref(),
            self.queue_repository.as_ref(),
            self.shutdown_scheduler.as_ref(),
            self.task_process_supervisor.as_ref(),
            self.application_control.as_ref(),
            self.diagnostics.as_ref(),
            events,
        )
    }
}
