use crate::application::app_error::AppResult;
use crate::application::FrontendEventPublisher;
use crate::application::ShutdownScheduler;

pub(crate) struct AutoShutdownPorts<'a> {
    shutdown_scheduler: &'a dyn ShutdownScheduler,
    events: &'a dyn FrontendEventPublisher,
}

impl<'a> AutoShutdownPorts<'a> {
    pub(crate) fn new(
        shutdown_scheduler: &'a dyn ShutdownScheduler,
        events: &'a dyn FrontendEventPublisher,
    ) -> Self {
        Self {
            shutdown_scheduler,
            events,
        }
    }

    pub(crate) fn cancel_auto_shutdown(&self) -> AppResult<()> {
        self.shutdown_scheduler.cancel_countdown()?;
        self.mark_auto_shutdown_cancelled();
        Ok(())
    }

    fn mark_auto_shutdown_cancelled(&self) {
        self.events.shutdown_countdown_cancelled();
    }
}
