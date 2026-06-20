use crate::application::task_snapshot::{TaskSnapshot, TaskStatusSnapshot};
use crate::domain::task::{Task, TaskStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StoredTaskStatus {
    Waiting,
    Downloading,
    Completed,
    Failed,
}

impl From<&TaskStatus> for StoredTaskStatus {
    fn from(status: &TaskStatus) -> Self {
        match status {
            TaskStatus::Waiting => Self::Waiting,
            TaskStatus::Downloading => Self::Downloading,
            TaskStatus::Completed => Self::Completed,
            TaskStatus::Failed => Self::Failed,
        }
    }
}

impl From<&TaskStatusSnapshot> for StoredTaskStatus {
    fn from(status: &TaskStatusSnapshot) -> Self {
        match status {
            TaskStatusSnapshot::Waiting => Self::Waiting,
            TaskStatusSnapshot::Downloading => Self::Downloading,
            TaskStatusSnapshot::Completed => Self::Completed,
            TaskStatusSnapshot::Failed => Self::Failed,
        }
    }
}

impl From<StoredTaskStatus> for TaskStatus {
    fn from(status: StoredTaskStatus) -> Self {
        match status {
            StoredTaskStatus::Waiting => Self::Waiting,
            StoredTaskStatus::Downloading => Self::Downloading,
            StoredTaskStatus::Completed => Self::Completed,
            StoredTaskStatus::Failed => Self::Failed,
        }
    }
}

impl From<StoredTaskStatus> for TaskStatusSnapshot {
    fn from(status: StoredTaskStatus) -> Self {
        match status {
            StoredTaskStatus::Waiting => Self::Waiting,
            StoredTaskStatus::Downloading => Self::Downloading,
            StoredTaskStatus::Completed => Self::Completed,
            StoredTaskStatus::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredTask {
    pub id: String,
    pub url: String,
    pub save_name: Option<String>,
    pub headers: Option<String>,
    pub status: StoredTaskStatus,
    pub retry_count: u8,
    pub progress: f32,
    pub speed: String,
    pub threads: String,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<&Task> for StoredTask {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
            status: StoredTaskStatus::from(&task.status),
            retry_count: task.retry_count,
            progress: 0.0,
            speed: String::new(),
            threads: String::new(),
            output_path: None,
            error_message: task.error_message.clone(),
            created_at: task.created_at,
        }
    }
}

impl From<&TaskSnapshot> for StoredTask {
    fn from(task: &TaskSnapshot) -> Self {
        Self {
            id: task.id.clone(),
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
            status: StoredTaskStatus::from(&task.status),
            retry_count: task.retry_count,
            progress: task.progress,
            speed: task.speed.clone(),
            threads: task.threads.clone(),
            output_path: task.output_path.clone(),
            error_message: task.error_message.clone(),
            created_at: task.created_at,
        }
    }
}

impl From<StoredTask> for Task {
    fn from(task: StoredTask) -> Self {
        Self {
            id: task.id,
            url: task.url,
            save_name: task.save_name,
            headers: task.headers,
            status: task.status.into(),
            retry_count: task.retry_count,
            error_message: task.error_message,
            created_at: task.created_at,
        }
    }
}

impl From<StoredTask> for TaskSnapshot {
    fn from(task: StoredTask) -> Self {
        Self {
            id: task.id,
            url: task.url,
            save_name: task.save_name,
            headers: task.headers,
            status: task.status.into(),
            retry_count: task.retry_count,
            progress: task.progress,
            speed: task.speed,
            threads: task.threads,
            output_path: task.output_path,
            error_message: task.error_message,
            created_at: task.created_at,
        }
    }
}

pub(crate) fn stored_tasks_from_domain(tasks: &[Task]) -> Vec<StoredTask> {
    tasks.iter().map(StoredTask::from).collect()
}

pub(crate) fn stored_tasks_from_snapshots(tasks: &[TaskSnapshot]) -> Vec<StoredTask> {
    tasks.iter().map(StoredTask::from).collect()
}

pub(crate) fn stored_tasks_into_domain(tasks: Vec<StoredTask>) -> Vec<Task> {
    tasks.into_iter().map(Task::from).collect()
}

pub(crate) fn stored_tasks_into_snapshots(tasks: Vec<StoredTask>) -> Vec<TaskSnapshot> {
    tasks.into_iter().map(TaskSnapshot::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_with_status(status: TaskStatus) -> Task {
        Task {
            id: "task-1".to_string(),
            url: "https://example.com/test.m3u8".to_string(),
            save_name: Some("sample".to_string()),
            headers: None,
            status,
            retry_count: 1,
            error_message: None,
            created_at: DateTime::from_timestamp(0, 0).expect("valid timestamp"),
        }
    }

    #[test]
    fn stored_task_preserves_task_status_json_shape() {
        let task = task_with_status(TaskStatus::Downloading);
        let value = serde_json::to_value(StoredTask::from(&task)).expect("serialize task record");

        assert_eq!(value["status"], serde_json::json!("downloading"));
        assert_eq!(value["saveName"], serde_json::json!("sample"));
        assert_eq!(value["retryCount"], serde_json::json!(1));
    }

    #[test]
    fn stored_task_round_trips_domain_task_fields() {
        let task = task_with_status(TaskStatus::Failed);
        let restored = Task::from(StoredTask::from(&task));

        assert_eq!(restored.id, task.id);
        assert_eq!(restored.status, TaskStatus::Failed);
        assert_eq!(restored.retry_count, task.retry_count);
        assert_eq!(restored.created_at, task.created_at);
    }
}
