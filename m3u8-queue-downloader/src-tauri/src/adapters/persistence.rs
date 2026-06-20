use crate::adapters::storage_files;
use crate::adapters::task_record::{
    stored_tasks_from_domain, stored_tasks_into_domain, StoredTask,
};
use crate::application::app_error::AppResult;
use crate::domain::queue::{
    QueueAggregate, QueueCurrentTask, QueuePendingHistory, QueueRunStatus, QueueTasks,
};
use crate::domain::retry_policy::RetryPolicy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const QUEUE_STATE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedQueueState {
    version: u32,
    state: PersistedQueueData,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedQueueData {
    tasks: Vec<StoredTask>,
    current_task_id: Option<String>,
    is_running: bool,
    #[serde(default)]
    pending_history_tasks: Vec<StoredTask>,
}

impl From<&QueueAggregate> for PersistedQueueData {
    fn from(state: &QueueAggregate) -> Self {
        Self {
            tasks: stored_tasks_from_domain(state.tasks()),
            current_task_id: state.current_task_id().map(str::to_string),
            is_running: state.is_running(),
            pending_history_tasks: stored_tasks_from_domain(state.pending_history_tasks()),
        }
    }
}

impl From<PersistedQueueData> for QueueAggregate {
    fn from(state: PersistedQueueData) -> Self {
        QueueAggregate::from_parts(
            QueueTasks::from_tasks(stored_tasks_into_domain(state.tasks)),
            QueueCurrentTask::from_task_id(state.current_task_id),
            QueueRunStatus::from_is_running(state.is_running),
            QueuePendingHistory::from_tasks(stored_tasks_into_domain(state.pending_history_tasks)),
            RetryPolicy::default(),
        )
    }
}

/// Handles saving and loading queue state to/from a JSON file
pub struct Persistence;

impl Persistence {
    /// Returns the default file path for queue state persistence
    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("m3u8-queue-downloader")
            .join("queue_state.json")
    }

    /// Save queue state to a JSON file
    pub fn save(state: &QueueAggregate, path: &Path) -> AppResult<()> {
        let persisted = PersistedQueueState {
            version: QUEUE_STATE_VERSION,
            state: state.into(),
        };
        let json = serde_json::to_string_pretty(&persisted)?;
        storage_files::write_atomic(path, json.as_bytes())
    }

    /// Load queue state from a JSON file. Returns None if file doesn't exist.
    pub fn load(path: &Path) -> Option<QueueAggregate> {
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(path).ok()?;
        let persisted: PersistedQueueState = serde_json::from_str(&content).ok()?;
        if persisted.version != QUEUE_STATE_VERSION {
            return None;
        }

        let mut state: QueueAggregate = persisted.state.into();
        state.normalize_after_restart();
        Some(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::queue::{QueueAggregate, QueueCurrentTask, QueuePendingHistory, QueueTasks};
    use crate::domain::retry_policy::RetryPolicy;
    use crate::domain::task::{Task, TaskStatus};
    use chrono::Utc;
    use uuid::Uuid;

    fn temp_state_path() -> PathBuf {
        std::env::temp_dir().join(format!("queue-state-{}.json", Uuid::new_v4()))
    }

    fn sample_state() -> QueueAggregate {
        QueueAggregate::from_parts(
            QueueTasks::from_tasks(vec![Task {
                id: "task-1".to_string(),
                url: "https://example.com/test.m3u8".to_string(),
                save_name: Some("sample".to_string()),
                headers: None,
                status: TaskStatus::Downloading,
                retry_count: 0,
                error_message: None,
                created_at: Utc::now(),
            }]),
            QueueCurrentTask::from_task_id(Some("task-1".to_string())),
            QueueRunStatus::Running,
            QueuePendingHistory::default(),
            RetryPolicy::default(),
        )
    }

    #[test]
    fn save_and_load_round_trip_with_restart_normalization() {
        let path = temp_state_path();
        Persistence::save(&sample_state(), &path).expect("save queue state");

        let loaded = Persistence::load(&path).expect("load queue state");
        assert_eq!(loaded.tasks().len(), 1);
        assert_eq!(loaded.tasks()[0].status, TaskStatus::Waiting);
        assert!(loaded.current_task_id().is_none());
        assert!(!loaded.is_running());

        std::fs::remove_file(path).expect("cleanup queue state");
    }

    #[test]
    fn load_ignores_unversioned_legacy_queue_state() {
        let path = temp_state_path();
        let legacy_json = r#"{"tasks":[],"currentTaskId":null,"isRunning":true}"#;
        std::fs::write(&path, legacy_json).expect("write legacy queue state");

        let loaded = Persistence::load(&path);

        assert!(loaded.is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_writes_versioned_queue_state_envelope() {
        let path = temp_state_path();
        Persistence::save(&sample_state(), &path).expect("save queue state");

        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read queue state"))
                .expect("parse saved queue state");

        assert_eq!(saved["version"], serde_json::json!(1));
        assert!(saved["state"].is_object());

        std::fs::remove_file(path).expect("cleanup queue state");
    }
}
