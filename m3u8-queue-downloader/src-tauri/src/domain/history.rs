use chrono::{DateTime, Utc};

#[cfg(test)]
use crate::domain::task::TaskStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryStatus {
    Completed,
    Failed,
}

/// An immutable record of a completed task lifecycle.
///
/// HistoryRecord is created when a task reaches a terminal state (Completed
/// or Failed). It captures the essential facts about what happened, without
/// carrying runtime progress fields or mutable state.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistoryRecord {
    task_id: String,
    url: String,
    save_name: Option<String>,
    status: HistoryStatus,
    error_message: Option<String>,
    artifact_ref: Option<String>,
    completed_at: DateTime<Utc>,
}

#[allow(dead_code)]
impl HistoryRecord {
    pub(crate) fn completed(
        task_id: String,
        url: String,
        save_name: Option<String>,
        artifact_ref: String,
        completed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            task_id,
            url,
            save_name,
            status: HistoryStatus::Completed,
            error_message: None,
            artifact_ref: Some(artifact_ref),
            completed_at,
        }
    }

    pub(crate) fn failed(
        task_id: String,
        url: String,
        save_name: Option<String>,
        error_message: String,
        completed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            task_id,
            url,
            save_name,
            status: HistoryStatus::Failed,
            error_message: Some(error_message),
            artifact_ref: None,
            completed_at,
        }
    }

    pub(crate) fn task_id(&self) -> &str {
        &self.task_id
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn save_name(&self) -> Option<&str> {
        self.save_name.as_deref()
    }

    pub(crate) fn status(&self) -> HistoryStatus {
        self.status
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn artifact_ref(&self) -> Option<&str> {
        self.artifact_ref.as_deref()
    }

    pub(crate) fn completed_at(&self) -> DateTime<Utc> {
        self.completed_at
    }
}

#[cfg(test)]
impl HistoryStatus {
    pub(crate) fn from_task_status(status: &TaskStatus) -> Option<Self> {
        match status {
            TaskStatus::Completed => Some(HistoryStatus::Completed),
            TaskStatus::Failed => Some(HistoryStatus::Failed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_status_is_derived_only_from_terminal_task_status() {
        assert_eq!(
            HistoryStatus::from_task_status(&TaskStatus::Completed),
            Some(HistoryStatus::Completed)
        );
        assert_eq!(
            HistoryStatus::from_task_status(&TaskStatus::Failed),
            Some(HistoryStatus::Failed)
        );
        assert_eq!(HistoryStatus::from_task_status(&TaskStatus::Waiting), None);
        assert_eq!(
            HistoryStatus::from_task_status(&TaskStatus::Downloading),
            None
        );
    }

    #[test]
    fn completed_record_has_artifact_and_no_error() {
        let record = HistoryRecord::completed(
            "task-1".to_string(),
            "https://example.com/test.m3u8".to_string(),
            Some("video".to_string()),
            "output.mp4".to_string(),
            DateTime::from_timestamp(1700000000, 0).expect("valid"),
        );

        assert_eq!(record.status(), HistoryStatus::Completed);
        assert_eq!(record.artifact_ref(), Some("output.mp4"));
        assert!(record.error_message().is_none());
    }

    #[test]
    fn failed_record_has_error_and_no_artifact() {
        let record = HistoryRecord::failed(
            "task-1".to_string(),
            "https://example.com/test.m3u8".to_string(),
            None,
            "network timeout".to_string(),
            DateTime::from_timestamp(1700000000, 0).expect("valid"),
        );

        assert_eq!(record.status(), HistoryStatus::Failed);
        assert_eq!(record.error_message(), Some("network timeout"));
        assert!(record.artifact_ref().is_none());
    }
}
