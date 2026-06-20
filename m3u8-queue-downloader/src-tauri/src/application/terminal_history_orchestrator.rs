use crate::application::app_error::AppResult;
use crate::application::queue_repository_outcomes::PrepareTaskFailureOutcome;
use crate::application::task_snapshot::TaskSnapshot;
use crate::application::HistoryRepository;
use crate::application::QueueRepository;

pub(crate) struct TerminalHistoryPorts<'a> {
    queue_repository: &'a dyn QueueRepository,
    history_repository: &'a dyn HistoryRepository,
}

impl<'a> TerminalHistoryPorts<'a> {
    pub(crate) fn new(
        queue_repository: &'a dyn QueueRepository,
        history_repository: &'a dyn HistoryRepository,
    ) -> Self {
        Self {
            queue_repository,
            history_repository,
        }
    }

    pub(crate) async fn handle_completed_task_history(
        &self,
        task_id: &str,
        output_path: &str,
    ) -> AppResult<Option<TaskSnapshot>> {
        let task = match self.stage_task_completion(task_id, output_path).await? {
            Some(task) => task,
            None => return Ok(None),
        };

        self.flush_pending_history_task(&task).await?;
        Ok(Some(task))
    }

    pub(crate) async fn handle_terminal_failure_task_history(
        &self,
        task_id: &str,
    ) -> AppResult<Option<TaskSnapshot>> {
        let task = match self.stage_terminal_history_task(task_id).await? {
            Some(task) => task,
            None => return Ok(None),
        };

        self.flush_pending_history_task(&task).await?;
        Ok(Some(task))
    }

    pub(crate) async fn handle_pending_history_flush(&self) -> AppResult<Vec<TaskSnapshot>> {
        let pending_tasks = self.pending_history_tasks().await;
        let mut flushed = Vec::new();

        for task in pending_tasks {
            self.flush_pending_history_task(&task).await?;
            flushed.push(task);
        }

        Ok(flushed)
    }

    pub(crate) async fn handle_task_failure_transition(
        &self,
        task_id: &str,
        error_message: &str,
    ) -> AppResult<PrepareTaskFailureOutcome> {
        self.queue_repository
            .prepare_task_failure(task_id, error_message)
            .await
    }

    pub(crate) async fn handle_task_failure_transition_error(
        &self,
        task_id: &str,
        error_message: &str,
    ) {
        self.queue_repository
            .pause_after_failure_persistence_error(task_id, error_message)
            .await;
    }

    async fn stage_task_completion(
        &self,
        task_id: &str,
        output_path: &str,
    ) -> AppResult<Option<TaskSnapshot>> {
        self.queue_repository
            .stage_task_completion(task_id, output_path)
            .await
    }

    async fn stage_terminal_history_task(&self, task_id: &str) -> AppResult<Option<TaskSnapshot>> {
        self.queue_repository
            .stage_terminal_history_task(task_id)
            .await
    }

    async fn pending_history_tasks(&self) -> Vec<TaskSnapshot> {
        self.queue_repository.pending_history_tasks().await
    }

    fn append_history_task(&self, task: &TaskSnapshot) -> AppResult<()> {
        self.history_repository.append(task)
    }

    async fn clear_pending_history_task(&self, task_id: &str) -> AppResult<bool> {
        self.queue_repository
            .clear_pending_history_task(task_id)
            .await
    }

    async fn flush_pending_history_task(&self, task: &TaskSnapshot) -> AppResult<()> {
        self.append_history_task(task)?;
        self.clear_pending_history_task(&task.id).await?;
        Ok(())
    }
}
