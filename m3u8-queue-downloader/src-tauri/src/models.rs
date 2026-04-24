use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CloseButtonBehavior {
    CloseToTray,
    Exit,
}

impl Default for CloseButtonBehavior {
    fn default() -> Self {
        Self::CloseToTray
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub close_button_behavior: CloseButtonBehavior,
    #[serde(default, rename = "autoShutdownOnComplete")]
    pub auto_action_on_complete: bool,
    #[serde(default, rename = "downloadDir")]
    pub download_dir: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_button_behavior: CloseButtonBehavior::CloseToTray,
            auto_action_on_complete: false,
            download_dir: None,
        }
    }
}

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
        }
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliOutputPage {
    pub lines: Vec<String>,
    pub offset: usize,
    pub total: usize,
    pub next_offset: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliTerminalState {
    pub committed_lines: Vec<String>,
    pub active_line: String,
    pub offset: usize,
    pub total: usize,
    pub has_more_before: bool,
}
