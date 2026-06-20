#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskFailureTransition {
    RetryScheduled,
    Terminal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrepareTaskFailureOutcome {
    Transition(TaskFailureTransition),
    Ignored,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueueRunStatus {
    Running,
    Paused,
}
