use crate::domain::retry_policy::{RetryDecision, RetryPolicy};
use crate::domain::task::{Task, TaskStatus};

#[derive(Debug, Clone)]
pub(crate) struct QueueAggregate {
    tasks: QueueTasks,
    current_task: QueueCurrentTask,
    run_status: QueueRunStatus,
    pending_history: QueuePendingHistory,
    #[allow(dead_code)]
    retry_policy: RetryPolicy,
}

pub(crate) enum StageTerminalHistoryResult {
    Staged(Task),
    Missing,
    InvalidStatus { status: TaskStatus },
}

pub(crate) enum StageTaskCompletionOutcome {
    Staged(Task),
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueueRunStatus {
    Running,
    Paused,
}

impl QueueRunStatus {
    pub(crate) fn is_running(self) -> bool {
        matches!(self, QueueRunStatus::Running)
    }

    pub(crate) fn from_is_running(is_running: bool) -> Self {
        if is_running {
            QueueRunStatus::Running
        } else {
            QueueRunStatus::Paused
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QueueCurrentTask {
    Assigned(String),
    None,
}

impl QueueCurrentTask {
    pub(crate) fn from_task_id(task_id: Option<String>) -> Self {
        match task_id {
            Some(task_id) => QueueCurrentTask::Assigned(task_id),
            None => QueueCurrentTask::None,
        }
    }

    fn task_id(&self) -> Option<&str> {
        match self {
            QueueCurrentTask::Assigned(task_id) => Some(task_id.as_str()),
            QueueCurrentTask::None => None,
        }
    }

    fn is_assigned(&self) -> bool {
        matches!(self, QueueCurrentTask::Assigned(_))
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct QueueTasks {
    tasks: Vec<Task>,
}

impl QueueTasks {
    pub(crate) fn from_tasks(tasks: Vec<Task>) -> Self {
        Self { tasks }
    }

    fn as_slice(&self) -> &[Task] {
        &self.tasks
    }

    fn push(&mut self, task: Task) {
        self.tasks.push(task);
    }

    fn has_live_work(&self) -> bool {
        self.tasks.iter().any(|task| task.status.is_live_work())
    }

    fn schedule_next_download(&mut self) -> Option<Task> {
        let task = self
            .tasks
            .iter_mut()
            .find(|task| task.status.is_waiting())?;
        task.status = TaskStatus::Downloading;
        Some(task.clone())
    }

    fn remove_task(&mut self, id: &str) -> RemoveTaskResult {
        let Some(position) = self.tasks.iter().position(|task| task.id == id) else {
            return RemoveTaskResult::Missing;
        };

        if !self.tasks[position].status.can_remove_from_queue() {
            return RemoveTaskResult::InvalidStatus {
                status: self.tasks[position].status.clone(),
            };
        }

        self.tasks.remove(position);
        RemoveTaskResult::Removed
    }

    fn retry_task(&mut self, id: &str) -> RetryTaskResult {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return RetryTaskResult::Missing;
        };

        if !task.status.is_failed() && !task.status.is_cancelled() {
            return RetryTaskResult::InvalidStatus {
                status: task.status.clone(),
            };
        }

        task.status = TaskStatus::Waiting;
        task.error_message = None;
        RetryTaskResult::Retried(task.clone())
    }

    fn stop_task(&mut self, id: &str) -> StopTaskResult {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return StopTaskResult::Missing;
        };

        if !task.status.is_downloading() {
            return StopTaskResult::NotDownloading {
                status: task.status.clone(),
            };
        }

        task.status = TaskStatus::Cancelled;
        task.error_message = Some("Stopped by user".to_string());
        StopTaskResult::Stopped(task.clone())
    }

    fn update_save_name(&mut self, id: &str, save_name: Option<String>) -> UpdateSaveNameResult {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return UpdateSaveNameResult::Missing;
        };

        if !task.status.is_waiting() {
            return UpdateSaveNameResult::NotWaiting {
                status: task.status.clone(),
            };
        }

        // 空字符串归一化为 None，与 add_task 入队时一致（恢复 CLI 自动识别）
        task.save_name = save_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        UpdateSaveNameResult::Updated
    }

    fn reorder_waiting_tasks(&mut self, task_ids: Vec<String>) {
        let original_tasks = std::mem::take(&mut self.tasks);
        let mut waiting_tasks: Vec<Task> = original_tasks
            .iter()
            .filter(|task| task.status.is_waiting())
            .cloned()
            .collect();
        let non_waiting: Vec<Task> = original_tasks
            .into_iter()
            .filter(|task| !task.status.is_waiting())
            .collect();

        let mut reordered_waiting = Vec::with_capacity(waiting_tasks.len());
        for id in &task_ids {
            if let Some(position) = waiting_tasks.iter().position(|task| &task.id == id) {
                reordered_waiting.push(waiting_tasks.remove(position));
            }
        }
        reordered_waiting.extend(waiting_tasks);

        self.tasks = non_waiting.into_iter().chain(reordered_waiting).collect();
    }

    fn prepare_task_failure(&mut self, id: &str, error_message: &str) -> PrepareTaskFailureOutcome {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return PrepareTaskFailureOutcome::Ignored;
        };

        match RetryPolicy::default().decide(task.retry_count) {
            RetryDecision::Retry { next_retry_count } => {
                task.retry_count = next_retry_count;
                task.status = TaskStatus::Waiting;
                task.error_message = None;
                PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
            }
            RetryDecision::Terminal => {
                task.status = TaskStatus::Failed;
                task.error_message = Some(error_message.to_string());
                PrepareTaskFailureOutcome::Transition(TaskFailureTransition::Terminal)
            }
        }
    }

    fn stage_task_completion(&mut self, id: &str) -> StageTaskCompletionOutcome {
        let Some(position) = self.tasks.iter().position(|task| task.id == id) else {
            return StageTaskCompletionOutcome::Missing;
        };
        let mut task = self.tasks.remove(position);
        task.status = TaskStatus::Completed;
        task.error_message = None;
        StageTaskCompletionOutcome::Staged(task)
    }

    fn stage_terminal_history_task(&mut self, id: &str) -> StageTerminalHistoryResult {
        let Some(position) = self.tasks.iter().position(|task| task.id == id) else {
            return StageTerminalHistoryResult::Missing;
        };

        if !self.tasks[position].status.is_failed() {
            return StageTerminalHistoryResult::InvalidStatus {
                status: self.tasks[position].status.clone(),
            };
        }

        StageTerminalHistoryResult::Staged(self.tasks.remove(position))
    }

    fn reset_downloading_tasks_for_exit(&mut self) {
        for task in &mut self.tasks {
            if task.status.is_downloading() {
                task.status = TaskStatus::Waiting;
            }
        }
    }

    fn pause_after_failure_persistence_error(&mut self, id: &str, error_message: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) {
            task.status = TaskStatus::Failed;
            task.error_message = Some(error_message.to_string());
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct QueuePendingHistory {
    tasks: Vec<Task>,
}

impl QueuePendingHistory {
    pub(crate) fn from_tasks(tasks: Vec<Task>) -> Self {
        Self { tasks }
    }

    fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    fn snapshot(&self) -> Vec<Task> {
        self.tasks.clone()
    }

    fn push_unique(&mut self, task: Task) {
        if !self.tasks.iter().any(|pending| pending.id == task.id) {
            self.tasks.push(task);
        }
    }

    fn clear_task(&mut self, id: &str) -> ClearPendingHistoryOutcome {
        if !self.tasks.iter().any(|task| task.id == id) {
            return ClearPendingHistoryOutcome::Missing;
        }

        self.tasks.retain(|task| task.id != id);
        ClearPendingHistoryOutcome::Cleared
    }

    fn retain_terminal_tasks(&mut self) {
        self.tasks
            .retain(|task| matches!(task.status, TaskStatus::Completed | TaskStatus::Failed));
    }
}

pub(crate) enum RemoveTaskResult {
    Removed,
    Missing,
    InvalidStatus { status: TaskStatus },
}

pub(crate) enum RetryTaskResult {
    Retried(Task),
    Missing,
    InvalidStatus { status: TaskStatus },
}

pub(crate) enum StopTaskResult {
    Stopped(Task),
    Missing,
    NotDownloading { status: TaskStatus },
}

pub(crate) enum UpdateSaveNameResult {
    Updated,
    Missing,
    NotWaiting { status: TaskStatus },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinishRunOutcome {
    Finished,
    StillActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClearPendingHistoryOutcome {
    Cleared,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AddTaskOutcome {
    ScheduleRequested,
    AddedWithoutScheduling,
}

#[derive(Debug, Clone)]
pub(crate) enum ScheduleNextTaskOutcome {
    Scheduled(Task),
    NoTaskReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskFailureTransition {
    RetryScheduled,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrepareTaskFailureOutcome {
    Transition(TaskFailureTransition),
    Ignored,
}

impl QueueAggregate {
    pub(crate) fn from_parts(
        tasks: QueueTasks,
        current_task: QueueCurrentTask,
        run_status: QueueRunStatus,
        pending_history: QueuePendingHistory,
        retry_policy: RetryPolicy,
    ) -> Self {
        Self {
            tasks,
            current_task,
            run_status,
            pending_history,
            retry_policy,
        }
    }

    pub(crate) fn add_task(&mut self, task: Task) -> AddTaskOutcome {
        self.tasks.push(task);
        if self.can_schedule_next() {
            AddTaskOutcome::ScheduleRequested
        } else {
            AddTaskOutcome::AddedWithoutScheduling
        }
    }

    pub(crate) fn can_schedule_next(&self) -> bool {
        self.is_running() && !self.has_current_task()
    }

    pub(crate) fn has_live_work(&self) -> bool {
        self.tasks.has_live_work()
    }

    pub(crate) fn finish_run_if_idle(&mut self) -> FinishRunOutcome {
        if self.is_running() && !self.has_live_work() && !self.has_current_task() {
            self.set_run_status(QueueRunStatus::Paused);
            return FinishRunOutcome::Finished;
        }

        FinishRunOutcome::StillActive
    }

    pub(crate) fn schedule_next(&mut self) -> ScheduleNextTaskOutcome {
        if !self.can_schedule_next() {
            return ScheduleNextTaskOutcome::NoTaskReady;
        }

        let Some(scheduled_task) = self.tasks.schedule_next_download() else {
            return ScheduleNextTaskOutcome::NoTaskReady;
        };
        self.assign_current_task(scheduled_task.id.clone());
        ScheduleNextTaskOutcome::Scheduled(scheduled_task)
    }

    pub(crate) fn remove_task(&mut self, id: &str) -> RemoveTaskResult {
        self.tasks.remove_task(id)
    }

    pub(crate) fn retry_task(&mut self, id: &str) -> RetryTaskResult {
        self.tasks.retry_task(id)
    }

    pub(crate) fn stop_task(&mut self, id: &str) -> StopTaskResult {
        let result = self.tasks.stop_task(id);
        if matches!(result, StopTaskResult::Stopped(_)) {
            self.clear_current_task_if_matches(id);
        }
        result
    }

    pub(crate) fn update_save_name(
        &mut self,
        id: &str,
        save_name: Option<String>,
    ) -> UpdateSaveNameResult {
        self.tasks.update_save_name(id, save_name)
    }

    pub(crate) fn reorder_waiting_tasks(&mut self, task_ids: Vec<String>) {
        self.tasks.reorder_waiting_tasks(task_ids);
    }

    pub(crate) fn prepare_task_failure(
        &mut self,
        id: &str,
        error_message: &str,
    ) -> PrepareTaskFailureOutcome {
        let transition = self.tasks.prepare_task_failure(id, error_message);
        self.clear_current_task_if_matches(id);
        transition
    }

    pub(crate) fn stage_task_completion(&mut self, id: &str) -> StageTaskCompletionOutcome {
        match self.tasks.stage_task_completion(id) {
            StageTaskCompletionOutcome::Staged(task) => {
                self.clear_current_task_if_matches(id);
                self.push_pending_history_task(task.clone());
                StageTaskCompletionOutcome::Staged(task)
            }
            StageTaskCompletionOutcome::Missing => StageTaskCompletionOutcome::Missing,
        }
    }

    pub(crate) fn stage_terminal_history_task(&mut self, id: &str) -> StageTerminalHistoryResult {
        match self.tasks.stage_terminal_history_task(id) {
            StageTerminalHistoryResult::Staged(task) => {
                self.clear_current_task_if_matches(id);
                self.push_pending_history_task(task.clone());
                StageTerminalHistoryResult::Staged(task)
            }
            StageTerminalHistoryResult::Missing => StageTerminalHistoryResult::Missing,
            StageTerminalHistoryResult::InvalidStatus { status } => {
                StageTerminalHistoryResult::InvalidStatus { status }
            }
        }
    }

    pub(crate) fn push_pending_history_task(&mut self, task: Task) {
        self.pending_history.push_unique(task);
    }

    pub(crate) fn pending_history_tasks(&self) -> &[Task] {
        self.pending_history.tasks()
    }

    pub(crate) fn pending_history_snapshot(&self) -> Vec<Task> {
        self.pending_history.snapshot()
    }

    pub(crate) fn clear_pending_history_task(&mut self, id: &str) -> ClearPendingHistoryOutcome {
        self.pending_history.clear_task(id)
    }

    pub(crate) fn set_run_status(&mut self, status: QueueRunStatus) {
        self.run_status = status;
    }

    pub(crate) fn is_running(&self) -> bool {
        self.run_status.is_running()
    }

    pub(crate) fn tasks(&self) -> &[Task] {
        self.tasks.as_slice()
    }

    pub(crate) fn current_task_id(&self) -> Option<&str> {
        self.current_task.task_id()
    }

    fn has_current_task(&self) -> bool {
        self.current_task.is_assigned()
    }

    fn is_current_task(&self, id: &str) -> bool {
        self.current_task_id() == Some(id)
    }

    fn assign_current_task(&mut self, id: String) {
        self.current_task = QueueCurrentTask::Assigned(id);
    }

    fn clear_current_task(&mut self) {
        self.current_task = QueueCurrentTask::None;
    }

    fn clear_current_task_if_matches(&mut self, id: &str) {
        if self.is_current_task(id) {
            self.clear_current_task();
        }
    }

    pub(crate) fn prepare_for_exit(&mut self) {
        self.set_run_status(QueueRunStatus::Paused);
        self.clear_current_task();
        self.tasks.reset_downloading_tasks_for_exit();
    }

    pub(crate) fn normalize_after_restart(&mut self) {
        self.prepare_for_exit();
        self.pending_history.retain_terminal_tasks();
    }

    pub(crate) fn pause_after_failure_persistence_error(&mut self, id: &str, error_message: &str) {
        self.set_run_status(QueueRunStatus::Paused);
        self.clear_current_task_if_matches(id);
        self.tasks
            .pause_after_failure_persistence_error(id, error_message);
    }
}

impl Default for QueueAggregate {
    fn default() -> Self {
        Self {
            tasks: QueueTasks::default(),
            current_task: QueueCurrentTask::None,
            run_status: QueueRunStatus::Paused,
            pending_history: QueuePendingHistory::default(),
            retry_policy: RetryPolicy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_with_id(id: &str) -> Task {
        Task::new_queued(
            id.to_string(),
            "https://example.com/test.m3u8".to_string(),
            None,
            None,
            chrono::DateTime::from_timestamp(0, 0).expect("valid timestamp"),
        )
    }

    fn task_with_status(status: TaskStatus) -> Task {
        let mut task = task_with_id("task-1");
        task.status = status;
        task
    }

    #[test]
    fn default_queue_state_is_paused_and_empty() {
        let state = QueueAggregate::default();

        assert!(state.tasks().is_empty());
        assert!(state.current_task_id().is_none());
        assert!(!state.is_running());
        assert!(state.pending_history_tasks().is_empty());
    }

    #[test]
    fn pending_history_tasks_are_deduplicated_by_task_id() {
        let mut state = QueueAggregate::default();

        state.push_pending_history_task(task_with_id("task-1"));
        state.push_pending_history_task(task_with_id("task-1"));

        assert_eq!(state.pending_history_tasks().len(), 1);
        assert_eq!(state.pending_history_tasks()[0].id, "task-1");
    }

    #[test]
    fn clear_pending_history_task_removes_matching_task_only() {
        let mut state = QueueAggregate::default();
        state.push_pending_history_task(task_with_id("task-1"));
        state.push_pending_history_task(task_with_id("task-2"));

        assert!(matches!(
            state.clear_pending_history_task("task-1"),
            ClearPendingHistoryOutcome::Cleared
        ));

        assert_eq!(state.pending_history_tasks().len(), 1);
        assert_eq!(state.pending_history_tasks()[0].id, "task-2");
    }

    #[test]
    fn clear_pending_history_task_reports_missing_without_mutation() {
        let mut state = QueueAggregate::default();
        state.push_pending_history_task(task_with_id("task-1"));

        assert!(matches!(
            state.clear_pending_history_task("missing"),
            ClearPendingHistoryOutcome::Missing
        ));

        assert_eq!(state.pending_history_tasks().len(), 1);
        assert_eq!(state.pending_history_tasks()[0].id, "task-1");
    }

    #[test]
    fn set_run_status_updates_queue_running_flag() {
        let mut state = QueueAggregate::default();

        state.set_run_status(QueueRunStatus::Running);
        assert!(state.is_running());

        state.set_run_status(QueueRunStatus::Paused);
        assert!(!state.is_running());
    }

    #[test]
    fn prepare_for_exit_pauses_queue_and_resets_downloading_tasks() {
        let mut downloading = task_with_id("downloading");
        downloading.status = TaskStatus::Downloading;
        let waiting = task_with_id("waiting");
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![downloading, waiting]),
            current_task: QueueCurrentTask::from_task_id(Some("downloading".to_string())),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        state.prepare_for_exit();

        assert!(!state.is_running());
        assert!(state.current_task_id().is_none());
        assert_eq!(state.tasks()[0].status, TaskStatus::Waiting);
        assert_eq!(state.tasks()[1].status, TaskStatus::Waiting);
    }

    #[test]
    fn prepare_for_exit_preserves_non_downloading_terminal_fields() {
        let mut failed = task_with_id("failed");
        failed.status = TaskStatus::Failed;
        failed.error_message = Some("network error".to_string());
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![failed]),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        state.prepare_for_exit();

        assert_eq!(state.tasks()[0].status, TaskStatus::Failed);
        assert_eq!(
            state.tasks()[0].error_message.as_deref(),
            Some("network error")
        );
    }

    #[test]
    fn normalize_after_restart_resets_runtime_state_and_keeps_terminal_history_only() {
        let mut downloading = task_with_id("downloading");
        downloading.status = TaskStatus::Downloading;
        let mut pending_completed = task_with_id("pending-completed");
        pending_completed.status = TaskStatus::Completed;
        let mut pending_failed = task_with_id("pending-failed");
        pending_failed.status = TaskStatus::Failed;
        let pending_waiting = task_with_id("pending-waiting");
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![downloading]),
            current_task: QueueCurrentTask::from_task_id(Some("downloading".to_string())),
            run_status: QueueRunStatus::Running,
            pending_history: QueuePendingHistory::from_tasks(vec![
                pending_completed,
                pending_failed,
                pending_waiting,
            ]),
            retry_policy: RetryPolicy::default(),
        };

        state.normalize_after_restart();

        assert!(!state.is_running());
        assert!(state.current_task_id().is_none());
        assert_eq!(state.tasks()[0].status, TaskStatus::Waiting);
        assert_eq!(state.pending_history_tasks().len(), 2);
        assert!(state
            .pending_history_tasks()
            .iter()
            .all(|task| matches!(task.status, TaskStatus::Completed | TaskStatus::Failed)));
    }

    #[test]
    fn pause_after_failure_persistence_error_releases_dead_active_task() {
        let mut task = task_with_id("task-1");
        task.status = TaskStatus::Downloading;
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task]),
            current_task: QueueCurrentTask::from_task_id(Some("task-1".to_string())),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        state.pause_after_failure_persistence_error("task-1", "disk full");

        assert!(!state.is_running());
        assert!(state.current_task_id().is_none());
        assert_eq!(state.tasks()[0].status, TaskStatus::Failed);
        assert_eq!(state.tasks()[0].error_message.as_deref(), Some("disk full"));
    }

    #[test]
    fn pause_after_failure_persistence_error_preserves_unrelated_current_task() {
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_status(TaskStatus::Downloading)]),
            current_task: QueueCurrentTask::from_task_id(Some("other".to_string())),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        state.pause_after_failure_persistence_error("missing", "disk full");

        assert!(!state.is_running());
        assert_eq!(state.current_task_id(), Some("other"));
        assert_eq!(state.tasks()[0].status, TaskStatus::Downloading);
    }

    #[test]
    fn add_task_returns_explicit_schedule_outcome() {
        let mut paused = QueueAggregate::default();
        assert!(matches!(
            paused.add_task(task_with_id("task-1")),
            AddTaskOutcome::AddedWithoutScheduling
        ));
        assert_eq!(paused.tasks().len(), 1);

        let mut running_idle = QueueAggregate {
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };
        assert!(matches!(
            running_idle.add_task(task_with_id("task-1")),
            AddTaskOutcome::ScheduleRequested
        ));

        let mut running_busy = QueueAggregate {
            run_status: QueueRunStatus::Running,
            current_task: QueueCurrentTask::from_task_id(Some("active".to_string())),
            ..QueueAggregate::default()
        };
        assert!(matches!(
            running_busy.add_task(task_with_id("task-1")),
            AddTaskOutcome::AddedWithoutScheduling
        ));
    }

    #[test]
    fn add_task_preserves_task_payload() {
        let task = task_with_id("task-1");
        let mut state = QueueAggregate::default();

        state.add_task(task.clone());

        assert_eq!(state.tasks()[0].id, task.id);
        assert_eq!(state.tasks()[0].url, task.url);
        assert_eq!(state.tasks()[0].status, TaskStatus::Waiting);
    }

    #[test]
    fn schedule_next_requires_running_queue_without_current_task() {
        let mut state = QueueAggregate::default();

        assert!(!state.can_schedule_next());

        state.set_run_status(QueueRunStatus::Running);
        assert!(state.can_schedule_next());

        state.assign_current_task("task-1".to_string());
        assert!(!state.can_schedule_next());
    }

    #[test]
    fn live_work_is_any_waiting_or_downloading_task() {
        let mut state = QueueAggregate::default();
        assert!(!state.has_live_work());

        state.tasks = QueueTasks::from_tasks(vec![task_with_status(TaskStatus::Completed)]);
        assert!(!state.has_live_work());

        state.tasks.push(task_with_status(TaskStatus::Waiting));
        assert!(state.has_live_work());
    }

    #[test]
    fn running_queue_finishes_only_when_idle() {
        let mut state = QueueAggregate {
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        assert!(matches!(
            state.finish_run_if_idle(),
            FinishRunOutcome::Finished
        ));
        assert!(!state.is_running());

        state.set_run_status(QueueRunStatus::Running);
        state.tasks = QueueTasks::from_tasks(vec![task_with_status(TaskStatus::Downloading)]);
        assert!(matches!(
            state.finish_run_if_idle(),
            FinishRunOutcome::StillActive
        ));
        assert!(state.is_running());
    }

    #[test]
    fn schedule_next_marks_first_waiting_task_as_current_download() {
        let mut first = task_with_id("task-1");
        let second = task_with_id("task-2");
        first.status = TaskStatus::Waiting;

        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![first, second]),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        let ScheduleNextTaskOutcome::Scheduled(scheduled) = state.schedule_next() else {
            panic!("task should schedule");
        };

        assert_eq!(scheduled.id, "task-1");
        assert_eq!(scheduled.status, TaskStatus::Downloading);
        assert_eq!(state.current_task_id(), Some("task-1"));
        assert_eq!(state.tasks()[0].status, TaskStatus::Downloading);
        assert_eq!(state.tasks()[1].status, TaskStatus::Waiting);
    }

    #[test]
    fn schedule_next_is_noop_when_paused_or_already_current() {
        let mut paused = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_status(TaskStatus::Waiting)]),
            ..QueueAggregate::default()
        };
        assert!(matches!(
            paused.schedule_next(),
            ScheduleNextTaskOutcome::NoTaskReady
        ));
        assert!(paused.current_task_id().is_none());
        assert_eq!(paused.tasks()[0].status, TaskStatus::Waiting);

        let mut busy = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_status(TaskStatus::Waiting)]),
            run_status: QueueRunStatus::Running,
            current_task: QueueCurrentTask::from_task_id(Some("other".to_string())),
            ..QueueAggregate::default()
        };
        assert!(matches!(
            busy.schedule_next(),
            ScheduleNextTaskOutcome::NoTaskReady
        ));
        assert_eq!(busy.current_task_id(), Some("other"));
        assert_eq!(busy.tasks()[0].status, TaskStatus::Waiting);
    }

    #[test]
    fn stage_task_completion_removes_task_and_tracks_pending_history() {
        let mut task = task_with_id("task-1");
        task.status = TaskStatus::Downloading;
        task.error_message = Some("old error".to_string());
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task]),
            current_task: QueueCurrentTask::from_task_id(Some("task-1".to_string())),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        let StageTaskCompletionOutcome::Staged(completed) =
            state.stage_task_completion("task-1")
        else {
            panic!("completion should stage");
        };

        assert!(state.tasks().is_empty());
        assert!(state.current_task_id().is_none());
        assert_eq!(completed.status, TaskStatus::Completed);
        assert!(completed.error_message.is_none());
        assert_eq!(state.pending_history_tasks().len(), 1);
        assert_eq!(state.pending_history_tasks()[0].id, "task-1");
    }

    #[test]
    fn stage_task_completion_is_noop_for_missing_task() {
        let mut state = QueueAggregate::default();

        assert!(matches!(
            state.stage_task_completion("missing"),
            StageTaskCompletionOutcome::Missing
        ));
        assert!(state.pending_history_tasks().is_empty());
    }

    #[test]
    fn stage_terminal_history_task_requires_failed_status() {
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_status(TaskStatus::Waiting)]),
            ..QueueAggregate::default()
        };

        let result = state.stage_terminal_history_task("task-1");

        assert!(matches!(
            result,
            StageTerminalHistoryResult::InvalidStatus {
                status: TaskStatus::Waiting
            }
        ));
        assert_eq!(state.tasks().len(), 1);
        assert!(state.pending_history_tasks().is_empty());
    }

    #[test]
    fn stage_terminal_history_task_removes_failed_task_and_tracks_pending_history() {
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_status(TaskStatus::Failed)]),
            current_task: QueueCurrentTask::from_task_id(Some("task-1".to_string())),
            ..QueueAggregate::default()
        };

        let result = state.stage_terminal_history_task("task-1");

        let StageTerminalHistoryResult::Staged(task) = result else {
            panic!("failed task should be staged");
        };
        assert_eq!(task.id, "task-1");
        assert_eq!(task.status, TaskStatus::Failed);
        assert!(state.tasks().is_empty());
        assert!(state.current_task_id().is_none());
        assert_eq!(state.pending_history_tasks().len(), 1);
        assert_eq!(state.pending_history_tasks()[0].id, "task-1");
    }

    #[test]
    fn stage_terminal_history_task_reports_missing_without_mutation() {
        let mut state = QueueAggregate::default();

        let result = state.stage_terminal_history_task("missing");

        assert!(matches!(result, StageTerminalHistoryResult::Missing));
        assert!(state.tasks().is_empty());
        assert!(state.pending_history_tasks().is_empty());
    }

    #[test]
    fn remove_task_allows_waiting_and_failed_tasks() {
        let mut failed = task_with_id("failed");
        failed.status = TaskStatus::Failed;
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_id("waiting"), failed]),
            ..QueueAggregate::default()
        };

        assert!(matches!(
            state.remove_task("waiting"),
            RemoveTaskResult::Removed
        ));
        assert_eq!(state.tasks().len(), 1);
        assert_eq!(state.tasks()[0].id, "failed");

        assert!(matches!(
            state.remove_task("failed"),
            RemoveTaskResult::Removed
        ));
        assert!(state.tasks().is_empty());
    }

    #[test]
    fn remove_task_rejects_active_or_completed_tasks_without_mutation() {
        let mut downloading = task_with_id("downloading");
        downloading.status = TaskStatus::Downloading;
        let mut completed = task_with_id("completed");
        completed.status = TaskStatus::Completed;
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![downloading, completed]),
            ..QueueAggregate::default()
        };

        assert!(matches!(
            state.remove_task("downloading"),
            RemoveTaskResult::InvalidStatus {
                status: TaskStatus::Downloading
            }
        ));
        assert!(matches!(
            state.remove_task("completed"),
            RemoveTaskResult::InvalidStatus {
                status: TaskStatus::Completed
            }
        ));
        assert_eq!(state.tasks().len(), 2);
    }

    #[test]
    fn remove_task_reports_missing_without_mutation() {
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_id("task-1")]),
            ..QueueAggregate::default()
        };

        assert!(matches!(
            state.remove_task("missing"),
            RemoveTaskResult::Missing
        ));
        assert_eq!(state.tasks().len(), 1);
    }

    #[test]
    fn update_save_name_updates_waiting_task() {
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_id("task-1")]),
            ..QueueAggregate::default()
        };

        let result = state.update_save_name("task-1", Some("my-video".to_string()));

        assert!(matches!(result, UpdateSaveNameResult::Updated));
        assert_eq!(state.tasks()[0].save_name.as_deref(), Some("my-video"));
    }

    #[test]
    fn update_save_name_rejects_non_waiting_task() {
        let mut downloading = task_with_id("downloading");
        downloading.status = TaskStatus::Downloading;
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![downloading]),
            ..QueueAggregate::default()
        };

        let result = state.update_save_name("downloading", Some("renamed".to_string()));

        assert!(matches!(
            result,
            UpdateSaveNameResult::NotWaiting {
                status: TaskStatus::Downloading
            }
        ));
        assert!(state.tasks()[0].save_name.is_none());
    }

    #[test]
    fn update_save_name_reports_missing_without_mutation() {
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_id("task-1")]),
            ..QueueAggregate::default()
        };

        let result = state.update_save_name("missing", Some("renamed".to_string()));

        assert!(matches!(result, UpdateSaveNameResult::Missing));
    }

    #[test]
    fn update_save_name_normalizes_empty_to_none() {
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_id("task-1")]),
            ..QueueAggregate::default()
        };

        // 先设非空名
        state.update_save_name("task-1", Some("named".to_string()));
        assert_eq!(state.tasks()[0].save_name.as_deref(), Some("named"));

        // 空字符串 / 纯空白 → 归一化为 None（恢复 CLI 自动识别）
        state.update_save_name("task-1", Some("   ".to_string()));
        assert!(state.tasks()[0].save_name.is_none());
    }

    #[test]
    fn retry_task_moves_failed_task_back_to_waiting() {
        let mut task = task_with_id("task-1");
        task.status = TaskStatus::Failed;
        task.error_message = Some("network error".to_string());
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task]),
            ..QueueAggregate::default()
        };

        let result = state.retry_task("task-1");

        let RetryTaskResult::Retried(task) = result else {
            panic!("failed task should retry");
        };
        assert_eq!(task.status, TaskStatus::Waiting);
        assert!(task.error_message.is_none());
        assert_eq!(state.tasks()[0].status, TaskStatus::Waiting);
    }

    #[test]
    fn retry_task_rejects_non_failed_tasks_without_mutation() {
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task_with_status(TaskStatus::Waiting)]),
            ..QueueAggregate::default()
        };

        let result = state.retry_task("task-1");

        assert!(matches!(
            result,
            RetryTaskResult::InvalidStatus {
                status: TaskStatus::Waiting
            }
        ));
        assert_eq!(state.tasks()[0].status, TaskStatus::Waiting);
    }

    #[test]
    fn retry_task_reports_missing_without_mutation() {
        let mut state = QueueAggregate::default();

        assert!(matches!(
            state.retry_task("missing"),
            RetryTaskResult::Missing
        ));
        assert!(state.tasks().is_empty());
    }

    #[test]
    fn retry_task_accepts_cancelled_status() {
        let mut task = task_with_id("task-1");
        task.status = TaskStatus::Cancelled;
        task.error_message = Some("Stopped by user".to_string());
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task]),
            ..QueueAggregate::default()
        };

        let result = state.retry_task("task-1");

        let RetryTaskResult::Retried(task) = result else {
            panic!("cancelled task should retry");
        };
        assert_eq!(task.status, TaskStatus::Waiting);
        assert!(task.error_message.is_none());
        assert_eq!(state.tasks()[0].status, TaskStatus::Waiting);
    }

    #[test]
    fn stop_task_marks_downloading_task_as_cancelled() {
        let mut task = task_with_id("task-1");
        task.status = TaskStatus::Downloading;
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task]),
            current_task: QueueCurrentTask::from_task_id(Some("task-1".to_string())),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        let result = state.stop_task("task-1");

        let StopTaskResult::Stopped(stopped) = result else {
            panic!("downloading task should stop");
        };
        assert_eq!(stopped.status, TaskStatus::Cancelled);
        assert_eq!(
            stopped.error_message.as_deref(),
            Some("Stopped by user")
        );
        assert!(state.current_task_id().is_none());
        assert_eq!(state.tasks()[0].status, TaskStatus::Cancelled);
    }

    #[test]
    fn stop_task_rejects_non_downloading_tasks_without_mutation() {
        let waiting = task_with_id("task-1");
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![waiting]),
            ..QueueAggregate::default()
        };

        let result = state.stop_task("task-1");

        assert!(matches!(
            result,
            StopTaskResult::NotDownloading {
                status: TaskStatus::Waiting
            }
        ));
        assert_eq!(state.tasks()[0].status, TaskStatus::Waiting);
    }

    #[test]
    fn stop_task_reports_missing_without_mutation() {
        let mut state = QueueAggregate::default();

        assert!(matches!(
            state.stop_task("missing"),
            StopTaskResult::Missing
        ));
        assert!(state.tasks().is_empty());
    }

    #[test]
    fn prepare_task_failure_schedules_retry_and_releases_current_task() {
        let mut task = task_with_id("task-1");
        task.status = TaskStatus::Downloading;
        task.error_message = Some("old error".to_string());
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task]),
            current_task: QueueCurrentTask::from_task_id(Some("task-1".to_string())),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        let transition = state.prepare_task_failure("task-1", "network error");

        assert_eq!(
            transition,
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)
        );
        assert!(state.current_task_id().is_none());
        assert_eq!(state.tasks()[0].retry_count, 1);
        assert_eq!(state.tasks()[0].status, TaskStatus::Waiting);
        assert!(state.tasks()[0].error_message.is_none());
    }

    #[test]
    fn prepare_task_failure_marks_terminal_failure_after_retry_limit() {
        let mut task = task_with_id("task-1");
        task.status = TaskStatus::Downloading;
        task.retry_count = 2;
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![task]),
            current_task: QueueCurrentTask::from_task_id(Some("task-1".to_string())),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        let transition = state.prepare_task_failure("task-1", "terminal error");

        assert_eq!(
            transition,
            PrepareTaskFailureOutcome::Transition(TaskFailureTransition::Terminal)
        );
        assert!(state.current_task_id().is_none());
        assert_eq!(state.tasks()[0].retry_count, 2);
        assert_eq!(state.tasks()[0].status, TaskStatus::Failed);
        assert_eq!(
            state.tasks()[0].error_message.as_deref(),
            Some("terminal error")
        );
    }

    #[test]
    fn prepare_task_failure_clears_stale_current_task_even_when_missing() {
        let mut state = QueueAggregate {
            current_task: QueueCurrentTask::from_task_id(Some("missing".to_string())),
            run_status: QueueRunStatus::Running,
            ..QueueAggregate::default()
        };

        let transition = state.prepare_task_failure("missing", "error");

        assert_eq!(transition, PrepareTaskFailureOutcome::Ignored);
        assert!(state.current_task_id().is_none());
        assert!(state.tasks().is_empty());
    }

    #[test]
    fn reorder_waiting_tasks_keeps_non_waiting_before_reordered_waiting_tasks() {
        let mut downloading = task_with_id("downloading");
        downloading.status = TaskStatus::Downloading;
        let first = task_with_id("first");
        let second = task_with_id("second");
        let third = task_with_id("third");
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![downloading, first, second, third]),
            ..QueueAggregate::default()
        };

        state.reorder_waiting_tasks(vec!["third".to_string(), "first".to_string()]);

        let ids = state
            .tasks()
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["downloading", "third", "first", "second"]);
        assert_eq!(state.tasks()[0].status, TaskStatus::Downloading);
    }

    #[test]
    fn reorder_waiting_tasks_ignores_unknown_ids_and_appends_unmentioned_waiting_tasks() {
        let first = task_with_id("first");
        let second = task_with_id("second");
        let mut state = QueueAggregate {
            tasks: QueueTasks::from_tasks(vec![first, second]),
            ..QueueAggregate::default()
        };

        state.reorder_waiting_tasks(vec!["missing".to_string(), "second".to_string()]);

        let ids = state
            .tasks()
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["second", "first"]);
    }
}
