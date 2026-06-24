use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Waiting,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub(crate) fn can_remove_from_queue(&self) -> bool {
        matches!(self, Self::Waiting | Self::Failed | Self::Cancelled)
    }

    pub(crate) fn is_failed(&self) -> bool {
        matches!(self, Self::Failed)
    }

    pub(crate) fn is_downloading(&self) -> bool {
        matches!(self, Self::Downloading)
    }

    pub(crate) fn is_live_work(&self) -> bool {
        matches!(self, Self::Waiting | Self::Downloading)
    }

    pub(crate) fn is_waiting(&self) -> bool {
        matches!(self, Self::Waiting)
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub url: String,
    pub save_name: Option<String>,
    pub headers: Option<String>,
    pub status: TaskStatus,
    pub retry_count: u8,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Task {
    pub(crate) fn new_queued(
        id: String,
        url: String,
        save_name: Option<String>,
        headers: Option<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            url,
            save_name,
            headers,
            status: TaskStatus::Waiting,
            retry_count: 0,
            error_message: None,
            created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_removal_is_allowed_only_for_non_active_tasks() {
        assert!(TaskStatus::Waiting.can_remove_from_queue());
        assert!(TaskStatus::Failed.can_remove_from_queue());
        assert!(TaskStatus::Cancelled.can_remove_from_queue());
        assert!(!TaskStatus::Downloading.can_remove_from_queue());
        assert!(!TaskStatus::Completed.can_remove_from_queue());
    }

    #[test]
    fn live_work_is_waiting_or_downloading() {
        assert!(TaskStatus::Waiting.is_live_work());
        assert!(TaskStatus::Downloading.is_live_work());
        assert!(!TaskStatus::Completed.is_live_work());
        assert!(!TaskStatus::Failed.is_live_work());
        assert!(!TaskStatus::Cancelled.is_live_work());
    }

    #[test]
    fn status_predicates_identify_exact_states() {
        assert!(TaskStatus::Waiting.is_waiting());
        assert!(TaskStatus::Downloading.is_downloading());
        assert!(TaskStatus::Failed.is_failed());
        assert!(TaskStatus::Cancelled.is_cancelled());
        assert!(!TaskStatus::Completed.is_waiting());
        assert!(!TaskStatus::Completed.is_downloading());
        assert!(!TaskStatus::Completed.is_failed());
        assert!(!TaskStatus::Completed.is_cancelled());
    }

    #[test]
    fn new_task_starts_waiting_with_no_terminal_fields() {
        let task = Task::new_queued(
            "task-1".to_string(),
            "https://example.com/test.m3u8".to_string(),
            None,
            None,
            DateTime::from_timestamp(0, 0).expect("valid timestamp"),
        );

        assert_eq!(task.status, TaskStatus::Waiting);
        assert_eq!(task.retry_count, 0);
        assert!(task.error_message.is_none());
    }
}
