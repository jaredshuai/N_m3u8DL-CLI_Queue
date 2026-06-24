use crate::application::query_models::{
    CliOutputPage, CliTerminalState, HistoryPage, QueueStateView, TaskStatusView, TaskView,
};
use crate::application::task_snapshot::TaskSnapshot;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatusDto {
    Waiting,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

impl From<TaskStatusView> for TaskStatusDto {
    fn from(status: TaskStatusView) -> Self {
        match status {
            TaskStatusView::Waiting => Self::Waiting,
            TaskStatusView::Downloading => Self::Downloading,
            TaskStatusView::Completed => Self::Completed,
            TaskStatusView::Failed => Self::Failed,
            TaskStatusView::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDto {
    pub id: String,
    pub url: String,
    pub save_name: Option<String>,
    pub headers: Option<String>,
    pub status: TaskStatusDto,
    pub retry_count: u8,
    pub progress: f32,
    pub speed: String,
    pub threads: String,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<&TaskView> for TaskDto {
    fn from(task: &TaskView) -> Self {
        Self {
            id: task.id.clone(),
            url: task.url.clone(),
            save_name: task.save_name.clone(),
            headers: task.headers.clone(),
            status: task.status.clone().into(),
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

impl From<TaskView> for TaskDto {
    fn from(task: TaskView) -> Self {
        Self::from(&task)
    }
}

impl From<TaskSnapshot> for TaskDto {
    fn from(task: TaskSnapshot) -> Self {
        TaskDto::from(TaskView::from(task))
    }
}

impl From<&TaskSnapshot> for TaskDto {
    fn from(task: &TaskSnapshot) -> Self {
        TaskDto::from(task.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStateDto {
    pub tasks: Vec<TaskDto>,
    pub current_task_id: Option<String>,
    pub is_running: bool,
}

impl From<QueueStateView> for QueueStateDto {
    fn from(state: QueueStateView) -> Self {
        Self {
            tasks: state.tasks.into_iter().map(TaskDto::from).collect(),
            current_task_id: state.current_task_id,
            is_running: state.is_running,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageDto {
    pub tasks: Vec<TaskDto>,
    pub has_more: bool,
    pub next_offset: usize,
}

impl From<HistoryPage> for HistoryPageDto {
    fn from(page: HistoryPage) -> Self {
        Self {
            tasks: page.tasks.into_iter().map(TaskDto::from).collect(),
            has_more: page.has_more,
            next_offset: page.next_offset,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliOutputPageDto {
    pub lines: Vec<String>,
    pub offset: usize,
    pub total: usize,
    pub next_offset: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
}

impl From<CliOutputPage> for CliOutputPageDto {
    fn from(page: CliOutputPage) -> Self {
        Self {
            lines: page.lines,
            offset: page.offset,
            total: page.total,
            next_offset: page.next_offset,
            has_more_before: page.has_more_before,
            has_more_after: page.has_more_after,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliTerminalStateDto {
    pub committed_lines: Vec<String>,
    pub active_line: String,
    pub offset: usize,
    pub total: usize,
    pub has_more_before: bool,
}

impl From<CliTerminalState> for CliTerminalStateDto {
    fn from(state: CliTerminalState) -> Self {
        Self {
            committed_lines: state.committed_lines,
            active_line: state.active_line,
            offset: state.offset,
            total: state.total,
            has_more_before: state.has_more_before,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_dto_preserves_frontend_json_shape() {
        let task = TaskView {
            id: "task-1".to_string(),
            url: "https://example.com/test.m3u8".to_string(),
            save_name: Some("sample".to_string()),
            headers: None,
            status: TaskStatusView::Downloading,
            retry_count: 1,
            progress: 0.5,
            speed: "1 MB/s".to_string(),
            threads: "8".to_string(),
            output_path: None,
            error_message: None,
            created_at: DateTime::from_timestamp(0, 0).expect("valid timestamp"),
        };

        let value = serde_json::to_value(TaskDto::from(task)).expect("serialize task dto");

        assert_eq!(value["status"], serde_json::json!("downloading"));
        assert_eq!(value["saveName"], serde_json::json!("sample"));
        assert_eq!(value["retryCount"], serde_json::json!(1));
    }

    #[test]
    fn terminal_state_dto_preserves_frontend_json_shape() {
        let state = CliTerminalState {
            committed_lines: vec!["line 1".to_string()],
            active_line: "active".to_string(),
            offset: 10,
            total: 11,
            has_more_before: true,
        };

        let value =
            serde_json::to_value(CliTerminalStateDto::from(state)).expect("serialize terminal dto");

        assert_eq!(value["committedLines"], serde_json::json!(["line 1"]));
        assert_eq!(value["activeLine"], serde_json::json!("active"));
        assert_eq!(value["hasMoreBefore"], serde_json::json!(true));
    }
}
