use crate::application::queue_state_snapshot::QueueStateSnapshot;
use crate::application::task_snapshot::{TaskSnapshot, TaskStatusSnapshot};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatusView {
    Waiting,
    Downloading,
    Completed,
    Failed,
}

impl From<TaskStatusSnapshot> for TaskStatusView {
    fn from(status: TaskStatusSnapshot) -> Self {
        match status {
            TaskStatusSnapshot::Waiting => Self::Waiting,
            TaskStatusSnapshot::Downloading => Self::Downloading,
            TaskStatusSnapshot::Completed => Self::Completed,
            TaskStatusSnapshot::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskView {
    pub id: String,
    pub url: String,
    pub save_name: Option<String>,
    pub headers: Option<String>,
    pub status: TaskStatusView,
    pub retry_count: u8,
    pub progress: f32,
    pub speed: String,
    pub threads: String,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<TaskSnapshot> for TaskView {
    fn from(task: TaskSnapshot) -> Self {
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

#[derive(Debug, Clone)]
pub struct QueueStateView {
    pub tasks: Vec<TaskView>,
    pub current_task_id: Option<String>,
    pub is_running: bool,
}

impl From<QueueStateSnapshot> for QueueStateView {
    fn from(state: QueueStateSnapshot) -> Self {
        Self {
            tasks: state.tasks.into_iter().map(TaskView::from).collect(),
            current_task_id: state.current_task_id,
            is_running: state.is_running,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistoryPage {
    pub tasks: Vec<TaskView>,
    pub has_more: bool,
    pub next_offset: usize,
}

#[derive(Debug, Clone)]
pub struct CliOutputPage {
    pub lines: Vec<String>,
    pub offset: usize,
    pub total: usize,
    pub next_offset: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
}

#[derive(Debug, Clone)]
pub struct CliTerminalState {
    pub committed_lines: Vec<String>,
    pub active_line: String,
    pub offset: usize,
    pub total: usize,
    pub has_more_before: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_output_page_represents_a_window_over_terminal_lines() {
        let page = CliOutputPage {
            lines: vec!["line 1".to_string(), "line 2".to_string()],
            offset: 10,
            total: 20,
            next_offset: 12,
            has_more_before: true,
            has_more_after: true,
        };

        assert_eq!(page.lines.len(), 2);
        assert_eq!(page.offset, 10);
        assert_eq!(page.next_offset, 12);
        assert!(page.has_more_before);
        assert!(page.has_more_after);
    }
}
