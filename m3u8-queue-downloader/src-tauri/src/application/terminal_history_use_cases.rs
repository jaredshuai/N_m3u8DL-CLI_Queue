use crate::application::app_error::AppResult;
use crate::application::task_snapshot::{TaskSnapshot, TaskStatusSnapshot};
use crate::application::terminal_history_orchestrator::TerminalHistoryPorts;
use crate::domain::history::HistoryStatus;

#[derive(Debug)]
pub(crate) enum TerminalHistoryRecordOutcome {
    Recorded(TaskSnapshot),
    Ignored,
}

#[derive(Debug)]
pub(crate) struct FlushedHistoryTask {
    pub(crate) status: HistoryStatus,
    pub(crate) task: TaskSnapshot,
}

pub(crate) async fn handle_completed_task_history(
    ports: &TerminalHistoryPorts<'_>,
    task_id: &str,
    output_path: &str,
    artifact_diagnostic: Option<&crate::application::artifact_resolution::ArtifactDiagnostic>,
) -> AppResult<TerminalHistoryRecordOutcome> {
    let task = ports
        .handle_completed_task_history(task_id, output_path, artifact_diagnostic)
        .await?;
    match task {
        Some(task) => Ok(TerminalHistoryRecordOutcome::Recorded(task)),
        None => Ok(TerminalHistoryRecordOutcome::Ignored),
    }
}

pub(crate) async fn handle_terminal_failure_task_history(
    ports: &TerminalHistoryPorts<'_>,
    task_id: &str,
) -> AppResult<TerminalHistoryRecordOutcome> {
    let task = ports.handle_terminal_failure_task_history(task_id).await?;
    match task {
        Some(task) => Ok(TerminalHistoryRecordOutcome::Recorded(task)),
        None => Ok(TerminalHistoryRecordOutcome::Ignored),
    }
}

pub(crate) async fn flush_pending_history_tasks(
    ports: &TerminalHistoryPorts<'_>,
) -> AppResult<Vec<FlushedHistoryTask>> {
    let flushed_tasks = ports.handle_pending_history_flush().await?;
    let mut result = Vec::new();
    for task in flushed_tasks {
        if let Some(status) = history_status_from_task_snapshot(&task) {
            result.push(FlushedHistoryTask { status, task });
        }
    }
    Ok(result)
}

fn history_status_from_task_snapshot(task: &TaskSnapshot) -> Option<HistoryStatus> {
    match &task.status {
        TaskStatusSnapshot::Completed => Some(HistoryStatus::Completed),
        TaskStatusSnapshot::Failed => Some(HistoryStatus::Failed),
        TaskStatusSnapshot::Waiting | TaskStatusSnapshot::Downloading | TaskStatusSnapshot::Cancelled => None,
    }
}
