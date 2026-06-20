use crate::application::app_error::AppResult;
use crate::application::run_completion_orchestrator::AutoShutdownPorts;
use crate::application::settings::CloseButtonBehavior;
use crate::application::ApplicationControl;
use crate::application::Diagnostics;
use crate::application::FrontendEventPublisher;
use crate::application::QueueRepository;
use crate::application::SettingsRepository;
use crate::application::ShutdownScheduler;
use crate::application::TaskProcessSupervisor;
use std::fmt::Display;

pub(crate) struct ExitPorts<'a> {
    settings_repository: &'a dyn SettingsRepository,
    queue_repository: &'a dyn QueueRepository,
    shutdown_scheduler: &'a dyn ShutdownScheduler,
    process_supervisor: &'a dyn TaskProcessSupervisor,
    application_control: &'a dyn ApplicationControl,
    diagnostics: &'a dyn Diagnostics,
    events: &'a dyn FrontendEventPublisher,
}

impl<'a> ExitPorts<'a> {
    pub(crate) fn new(
        settings_repository: &'a dyn SettingsRepository,
        queue_repository: &'a dyn QueueRepository,
        shutdown_scheduler: &'a dyn ShutdownScheduler,
        process_supervisor: &'a dyn TaskProcessSupervisor,
        application_control: &'a dyn ApplicationControl,
        diagnostics: &'a dyn Diagnostics,
        events: &'a dyn FrontendEventPublisher,
    ) -> Self {
        Self {
            settings_repository,
            queue_repository,
            shutdown_scheduler,
            process_supervisor,
            application_control,
            diagnostics,
            events,
        }
    }

    pub(crate) fn close_button_behavior(&self) -> CloseButtonBehavior {
        self.settings_repository.get().close_button_behavior
    }

    pub(crate) fn cancel_auto_shutdown(&self) -> AppResult<()> {
        let ports = AutoShutdownPorts::new(self.shutdown_scheduler, self.events);
        ports.cancel_auto_shutdown()
    }

    pub(crate) fn hide_main_window(&self) -> AppResult<()> {
        self.application_control.hide_main_window()
    }

    pub(crate) async fn exit_application(&self) -> AppResult<()> {
        self.begin_shutdown().await;
        if let Err(err) = self.prepare_for_exit().await {
            self.mark_exit_queue_state_persistence_failed(&err);
        }
        if let Err(err) = self.terminate_all_running_processes().await {
            self.mark_exit_running_processes_termination_failed(&err);
        }
        self.exit(0);
        Ok(())
    }

    async fn begin_shutdown(&self) {
        self.process_supervisor.begin_shutdown().await;
    }

    async fn prepare_for_exit(&self) -> AppResult<()> {
        self.queue_repository.prepare_for_exit().await
    }

    async fn terminate_all_running_processes(&self) -> AppResult<()> {
        self.process_supervisor
            .terminate_all_running_processes()
            .await
    }

    fn mark_exit_queue_state_persistence_failed(&self, error: &impl Display) {
        self.diagnostics.warn(&format!(
            "Failed to persist queue state before exit: {}",
            error
        ));
    }

    fn mark_exit_running_processes_termination_failed(&self, error: &impl Display) {
        self.diagnostics.warn(&format!(
            "Failed to terminate running processes during exit: {}",
            error
        ));
    }

    fn exit(&self, code: i32) {
        self.application_control.exit(code);
    }
}
