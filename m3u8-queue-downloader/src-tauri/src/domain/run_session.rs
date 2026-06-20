use chrono::{DateTime, Utc};

/// A single execution session of the download queue.
///
/// Each time the queue transitions from Paused to Running, a new RunSession
/// begins. When the queue finishes or is paused, the session ends.
/// Sessions provide identity and temporal context for task execution history.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunSession {
    id: String,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    status: RunSessionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunSessionStatus {
    Active,
    Ended,
}

#[allow(dead_code)]
impl RunSession {
    pub(crate) fn new(id: String, started_at: DateTime<Utc>) -> Self {
        Self {
            id,
            started_at,
            ended_at: None,
            status: RunSessionStatus::Active,
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub(crate) fn ended_at(&self) -> Option<DateTime<Utc>> {
        self.ended_at
    }

    pub(crate) fn is_active(&self) -> bool {
        self.status == RunSessionStatus::Active
    }

    pub(crate) fn end(&mut self, ended_at: DateTime<Utc>) -> EndRunSessionOutcome {
        if self.status == RunSessionStatus::Ended {
            return EndRunSessionOutcome::AlreadyEnded;
        }
        self.status = RunSessionStatus::Ended;
        self.ended_at = Some(ended_at);
        EndRunSessionOutcome::Ended
    }

    pub(crate) fn duration(&self) -> Option<chrono::Duration> {
        self.ended_at.map(|end| end - self.started_at)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndRunSessionOutcome {
    Ended,
    AlreadyEnded,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        DateTime::from_timestamp(1700000000, 0).expect("valid timestamp")
    }

    #[test]
    fn new_session_is_active_with_no_end() {
        let session = RunSession::new("session-1".to_string(), fixed_time());
        assert!(session.is_active());
        assert_eq!(session.id(), "session-1");
        assert_eq!(session.started_at(), fixed_time());
        assert!(session.ended_at().is_none());
        assert!(session.duration().is_none());
    }

    #[test]
    fn ending_session_marks_it_ended_with_timestamp() {
        let mut session = RunSession::new("session-1".to_string(), fixed_time());
        let end_time = DateTime::from_timestamp(1700000100, 0).expect("valid timestamp");

        let outcome = session.end(end_time);

        assert_eq!(outcome, EndRunSessionOutcome::Ended);
        assert!(!session.is_active());
        assert_eq!(session.ended_at(), Some(end_time));
    }

    #[test]
    fn ending_already_ended_session_is_idempotent() {
        let mut session = RunSession::new("session-1".to_string(), fixed_time());
        let end_time = DateTime::from_timestamp(1700000100, 0).expect("valid timestamp");

        session.end(end_time);
        let outcome = session.end(end_time);

        assert_eq!(outcome, EndRunSessionOutcome::AlreadyEnded);
    }

    #[test]
    fn duration_is_computed_from_start_to_end() {
        let mut session = RunSession::new("session-1".to_string(), fixed_time());
        let end_time = DateTime::from_timestamp(1700000100, 0).expect("valid timestamp");

        session.end(end_time);

        let duration = session.duration().expect("should have duration");
        assert_eq!(duration.num_seconds(), 100);
    }
}
