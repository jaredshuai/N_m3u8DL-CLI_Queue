use crate::application::task_snapshot::TaskSnapshot;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ExitedChildFailureOutcome {
    RetryScheduled,
    Terminal,
    Ignored,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScheduleNextOutcome {
    QueueChanged,
    QueueUnchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduleNextRequest {
    Requested,
    NotRequested,
}

#[derive(Debug)]
pub(crate) enum StartFailureOutcome {
    RetryScheduled,
    Terminal(TaskSnapshot),
    Ignored,
}

impl From<bool> for ScheduleNextRequest {
    fn from(schedule_requested: bool) -> Self {
        if schedule_requested {
            Self::Requested
        } else {
            Self::NotRequested
        }
    }
}
