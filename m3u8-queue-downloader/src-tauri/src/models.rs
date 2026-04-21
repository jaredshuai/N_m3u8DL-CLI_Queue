use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Status of a download task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Waiting,
    Downloading,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HistoryStatus {
    Completed,
    Failed,
}

impl HistoryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HistoryStatus::Completed => "completed",
            HistoryStatus::Failed => "failed",
        }
    }

    pub fn from_task_status(status: &TaskStatus) -> Result<Self, String> {
        match status {
            TaskStatus::Completed => Ok(HistoryStatus::Completed),
            TaskStatus::Failed => Ok(HistoryStatus::Failed),
            _ => Err("Only completed and failed tasks can be stored in history".to_string()),
        }
    }
}

/// A single download task in the queue
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub url: String,
    pub save_name: Option<String>,
    pub headers: Option<String>,
    pub status: TaskStatus,
    pub retry_count: u8,
    pub progress: f32,
    pub speed: String,
    pub threads: String,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub log_lines: VecDeque<String>,
}

impl Task {
    pub fn new(url: String, save_name: Option<String>, headers: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            url,
            save_name,
            headers,
            status: TaskStatus::Waiting,
            retry_count: 0,
            progress: 0.0,
            speed: String::new(),
            threads: String::new(),
            output_path: None,
            error_message: None,
            created_at: Utc::now(),
            log_lines: VecDeque::new(),
        }
    }
}

/// Overall state of the download queue
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    pub tasks: Vec<Task>,
    pub current_task_id: Option<String>,
    pub is_running: bool,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            current_task_id: None,
            is_running: false,
        }
    }
}

/// Payload for adding a new task from the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTaskPayload {
    pub url: String,
    pub save_name: Option<String>,
    pub headers: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub tasks: Vec<Task>,
    pub has_more: bool,
    pub next_offset: usize,
}
