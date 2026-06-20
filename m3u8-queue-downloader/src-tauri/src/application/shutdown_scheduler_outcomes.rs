#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShutdownResetOutcome {
    CountdownCancelled,
    NoCountdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShutdownCountdownStartDecision {
    StartAllowed,
    Blocked,
}
