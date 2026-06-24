use crate::application::artifact_resolution::ArtifactDiagnostic;
use crate::domain::task::TaskStatus;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TaskStatusSnapshot {
    Waiting,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

impl From<&TaskStatus> for TaskStatusSnapshot {
    fn from(status: &TaskStatus) -> Self {
        match status {
            TaskStatus::Waiting => Self::Waiting,
            TaskStatus::Downloading => Self::Downloading,
            TaskStatus::Completed => Self::Completed,
            TaskStatus::Failed => Self::Failed,
            TaskStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TaskSnapshot {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) save_name: Option<String>,
    pub(crate) headers: Option<String>,
    pub(crate) status: TaskStatusSnapshot,
    pub(crate) retry_count: u8,
    pub(crate) error_message: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) progress: f32,
    pub(crate) speed: String,
    pub(crate) threads: String,
    pub(crate) output_path: Option<String>,
    /// Why `output_path` is `None` for a completed task, when the artifact
    /// inventory itself failed (permission denied / IO error / ...).
    /// `None` for non-completed tasks or completed-with-path. Persisted via
    /// `StoredArtifactDiagnostic` mirror. See ADR-0005 decision 6.
    pub(crate) artifact_diagnostic: Option<ArtifactDiagnostic>,
}

impl From<&crate::domain::task::Task> for TaskSnapshot {
    fn from(task: &crate::domain::task::Task) -> Self {
        Self {
            id: task.id.clone(),
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
            status: TaskStatusSnapshot::from(&task.status),
            retry_count: task.retry_count,
            error_message: task.error_message.clone(),
            created_at: task.created_at,
            progress: 0.0,
            speed: String::new(),
            threads: String::new(),
            output_path: None,
            artifact_diagnostic: None,
        }
    }
}

impl From<crate::domain::task::Task> for TaskSnapshot {
    fn from(task: crate::domain::task::Task) -> Self {
        Self::from(&task)
    }
}

impl TaskSnapshot {
    pub(crate) fn from_task_and_runtime(
        task: &crate::domain::task::Task,
        runtime: &crate::application::task_runtime_state::TaskRuntimeState,
    ) -> Self {
        Self {
            id: task.id.clone(),
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
            status: TaskStatusSnapshot::from(&task.status),
            retry_count: task.retry_count,
            error_message: task.error_message.clone(),
            created_at: task.created_at,
            progress: runtime.progress,
            speed: runtime.speed.clone(),
            threads: runtime.threads.clone(),
            output_path: runtime.output_path.clone(),
            artifact_diagnostic: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn waiting(task: &crate::domain::task::Task) -> Self {
        Self {
            id: task.id.clone(),
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
            status: TaskStatusSnapshot::Waiting,
            retry_count: task.retry_count,
            error_message: task.error_message.clone(),
            created_at: task.created_at,
            progress: 0.0,
            speed: String::new(),
            threads: String::new(),
            output_path: None,
            artifact_diagnostic: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn failed(task: &crate::domain::task::Task) -> Self {
        Self {
            id: task.id.clone(),
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
            status: TaskStatusSnapshot::Failed,
            retry_count: task.retry_count,
            error_message: task.error_message.clone(),
            created_at: task.created_at,
            progress: 0.0,
            speed: String::new(),
            threads: String::new(),
            output_path: None,
            artifact_diagnostic: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn completed(task: &crate::domain::task::Task, output_path: &str) -> Self {
        Self {
            id: task.id.clone(),
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
            status: TaskStatusSnapshot::Completed,
            retry_count: task.retry_count,
            error_message: task.error_message.clone(),
            created_at: task.created_at,
            progress: 1.0,
            speed: String::new(),
            threads: String::new(),
            output_path: Some(output_path.to_string()),
            artifact_diagnostic: None,
        }
    }
}
