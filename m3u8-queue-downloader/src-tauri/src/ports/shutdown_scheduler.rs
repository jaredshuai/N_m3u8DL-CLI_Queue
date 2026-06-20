use crate::application::app_error::AppResult;
use crate::application::shutdown_scheduler_outcomes::{
    ShutdownCountdownStartDecision, ShutdownResetOutcome,
};
use std::sync::Arc;

pub(crate) trait ShutdownScheduler: Send + Sync {
    fn reset_for_new_run(&self) -> AppResult<ShutdownResetOutcome>;
    fn mark_run_failure(&self);
    fn clear_cancellation_after_reenable(&self);
    fn countdown_start_decision(&self) -> ShutdownCountdownStartDecision;
    fn start_countdown(&self) -> AppResult<u64>;
    fn cancel_countdown(&self) -> AppResult<()>;
}

impl<T> ShutdownScheduler for Arc<T>
where
    T: ShutdownScheduler + ?Sized,
{
    fn reset_for_new_run(&self) -> AppResult<ShutdownResetOutcome> {
        self.as_ref().reset_for_new_run()
    }

    fn mark_run_failure(&self) {
        self.as_ref().mark_run_failure();
    }

    fn clear_cancellation_after_reenable(&self) {
        self.as_ref().clear_cancellation_after_reenable();
    }

    fn countdown_start_decision(&self) -> ShutdownCountdownStartDecision {
        self.as_ref().countdown_start_decision()
    }

    fn start_countdown(&self) -> AppResult<u64> {
        self.as_ref().start_countdown()
    }

    fn cancel_countdown(&self) -> AppResult<()> {
        self.as_ref().cancel_countdown()
    }
}
