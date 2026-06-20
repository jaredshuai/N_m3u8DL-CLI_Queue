#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessRunnerShutdownStatus {
    Running,
    ShuttingDown,
}
