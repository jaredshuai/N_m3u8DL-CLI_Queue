#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessRunnerShutdownStatus {
    Running,
    ShuttingDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskTerminationClaim {
    pub(crate) task_id: String,
    pub(crate) generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskTerminationClaimOutcome {
    Claimed(TaskTerminationClaim),
    AlreadyClaimed,
    AlreadyExited,
}
