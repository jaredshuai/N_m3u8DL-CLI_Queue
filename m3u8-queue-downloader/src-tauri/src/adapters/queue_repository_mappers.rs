use crate::application::app_error::{AppError, AppResult};
use crate::application::queue_repository_outcomes::{
    PrepareTaskFailureOutcome as ApplicationPrepareTaskFailureOutcome,
    QueueRunStatus as ApplicationQueueRunStatus,
    TaskFailureTransition as ApplicationTaskFailureTransition,
};
use crate::application::task_snapshot::TaskSnapshot;
use crate::domain::queue::{
    AddTaskOutcome as DomainAddTaskOutcome, ClearPendingHistoryOutcome, FinishRunOutcome,
    PrepareTaskFailureOutcome as DomainPrepareTaskFailureOutcome,
    QueueRunStatus as DomainQueueRunStatus, RemoveTaskResult, RetryTaskResult,
    ScheduleNextTaskOutcome, StageTaskCompletionOutcome as DomainStageTaskCompletionOutcome,
    StageTerminalHistoryResult, StopTaskResult, TaskFailureTransition as DomainTaskFailureTransition,
    UpdateSaveNameResult,
};

pub(crate) fn domain_run_status(status: ApplicationQueueRunStatus) -> DomainQueueRunStatus {
    match status {
        ApplicationQueueRunStatus::Running => DomainQueueRunStatus::Running,
        ApplicationQueueRunStatus::Paused => DomainQueueRunStatus::Paused,
    }
}

pub(crate) fn application_add_task_outcome(outcome: DomainAddTaskOutcome) -> bool {
    matches!(outcome, DomainAddTaskOutcome::ScheduleRequested)
}

pub(crate) fn application_prepare_task_failure_outcome(
    outcome: DomainPrepareTaskFailureOutcome,
) -> ApplicationPrepareTaskFailureOutcome {
    match outcome {
        DomainPrepareTaskFailureOutcome::Transition(
            DomainTaskFailureTransition::RetryScheduled,
        ) => ApplicationPrepareTaskFailureOutcome::Transition(
            ApplicationTaskFailureTransition::RetryScheduled,
        ),
        DomainPrepareTaskFailureOutcome::Transition(DomainTaskFailureTransition::Terminal) => {
            ApplicationPrepareTaskFailureOutcome::Transition(
                ApplicationTaskFailureTransition::Terminal,
            )
        }
        DomainPrepareTaskFailureOutcome::Ignored => ApplicationPrepareTaskFailureOutcome::Ignored,
    }
}

pub(crate) fn application_run_finish_outcome(outcome: FinishRunOutcome) -> bool {
    matches!(outcome, FinishRunOutcome::Finished)
}

pub(crate) fn application_pending_history_clear_outcome(
    outcome: ClearPendingHistoryOutcome,
) -> bool {
    matches!(outcome, ClearPendingHistoryOutcome::Cleared)
}

pub(crate) fn application_schedule_next_outcome(
    outcome: ScheduleNextTaskOutcome,
) -> Option<TaskSnapshot> {
    match outcome {
        ScheduleNextTaskOutcome::Scheduled(task) => Some(TaskSnapshot::from(task)),
        ScheduleNextTaskOutcome::NoTaskReady => None,
    }
}

pub(crate) fn application_task_completion_staging_outcome(
    outcome: DomainStageTaskCompletionOutcome,
) -> Option<TaskSnapshot> {
    match outcome {
        DomainStageTaskCompletionOutcome::Staged(task) => Some(TaskSnapshot::from(task)),
        DomainStageTaskCompletionOutcome::Missing => None,
    }
}

pub(crate) fn application_terminal_history_staging_outcome(
    id: &str,
    outcome: StageTerminalHistoryResult,
) -> AppResult<Option<TaskSnapshot>> {
    match outcome {
        StageTerminalHistoryResult::Staged(task) => Ok(Some(TaskSnapshot::from(task))),
        StageTerminalHistoryResult::Missing => Ok(None),
        StageTerminalHistoryResult::InvalidStatus { status } => Err(AppError::InvalidTaskStatus {
            action: "record terminal history",
            id: id.to_string(),
            status: format!("{:?}", status),
        }),
    }
}

pub(crate) fn application_remove_task_result(id: &str, result: RemoveTaskResult) -> AppResult<()> {
    match result {
        RemoveTaskResult::Removed => Ok(()),
        RemoveTaskResult::Missing => Err(AppError::TaskNotFound { id: id.to_string() }),
        RemoveTaskResult::InvalidStatus { status } => Err(AppError::InvalidTaskStatus {
            action: "remove",
            id: id.to_string(),
            status: format!("{:?}", status),
        }),
    }
}

pub(crate) fn application_update_save_name_result(
    id: &str,
    result: UpdateSaveNameResult,
) -> AppResult<()> {
    match result {
        UpdateSaveNameResult::Updated => Ok(()),
        UpdateSaveNameResult::Missing => Err(AppError::TaskNotFound { id: id.to_string() }),
        UpdateSaveNameResult::NotWaiting { status } => Err(AppError::InvalidTaskStatus {
            action: "rename",
            id: id.to_string(),
            status: format!("{:?}", status),
        }),
    }
}

pub(crate) fn application_retry_task_result(
    id: &str,
    result: RetryTaskResult,
) -> AppResult<TaskSnapshot> {
    match result {
        RetryTaskResult::Retried(task) => Ok(TaskSnapshot::from(task)),
        RetryTaskResult::Missing => Err(AppError::TaskNotFound { id: id.to_string() }),
        RetryTaskResult::InvalidStatus { status } => Err(AppError::InvalidTaskStatus {
            action: "retry",
            id: id.to_string(),
            status: format!("{:?}", status),
        }),
    }
}

pub(crate) fn application_stop_task_result(id: &str, result: StopTaskResult) -> AppResult<()> {
    match result {
        StopTaskResult::Stopped(_) => Ok(()),
        StopTaskResult::Missing => Err(AppError::TaskNotFound { id: id.to_string() }),
        StopTaskResult::NotDownloading { status } => Err(AppError::InvalidTaskStatus {
            action: "stop",
            id: id.to_string(),
            status: format!("{:?}", status),
        }),
    }
}
