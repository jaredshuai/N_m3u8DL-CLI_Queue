use crate::application::app_error::AppResult;
use crate::application::queue_scheduling_orchestrator::QueueSchedulingPorts;
use crate::application::shutdown_scheduler_outcomes::ShutdownResetOutcome;
use crate::application::FrontendEventPublisher;
use crate::application::ShutdownScheduler;

pub(crate) struct QueueStartPorts<'a> {
    scheduling_ports: QueueSchedulingPorts<'a>,
    shutdown_scheduler: &'a dyn ShutdownScheduler,
    events: &'a dyn FrontendEventPublisher,
}

impl<'a> QueueStartPorts<'a> {
    pub(crate) fn new(
        scheduling_ports: QueueSchedulingPorts<'a>,
        shutdown_scheduler: &'a dyn ShutdownScheduler,
        events: &'a dyn FrontendEventPublisher,
    ) -> Self {
        Self {
            scheduling_ports,
            shutdown_scheduler,
            events,
        }
    }

    fn begin_queue_start_attempt(&self) -> AppResult<()> {
        let outcome = self.shutdown_scheduler.reset_for_new_run()?;
        self.handle_shutdown_reset_outcome(outcome);
        Ok(())
    }

    pub(crate) async fn handle_queue_start(&self) -> AppResult<()> {
        self.begin_queue_start_attempt()?;
        self.scheduling_ports.handle_queue_start().await
    }

    fn handle_shutdown_reset_outcome(&self, outcome: ShutdownResetOutcome) {
        match outcome {
            ShutdownResetOutcome::CountdownCancelled => {
                self.mark_queue_start_shutdown_countdown_cancelled();
            }
            ShutdownResetOutcome::NoCountdown => {}
        }
    }

    fn mark_queue_start_shutdown_countdown_cancelled(&self) {
        self.events.shutdown_countdown_cancelled();
    }
}
