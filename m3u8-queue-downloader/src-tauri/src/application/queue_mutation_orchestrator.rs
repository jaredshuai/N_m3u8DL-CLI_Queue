use crate::application::app_error::AppResult;
use crate::application::queue_repository_outcomes::QueueRunStatus;
use crate::application::FrontendEventPublisher;
use crate::application::QueueRepository;

pub(crate) struct QueueMutationPorts<'a> {
    queue_repository: &'a dyn QueueRepository,
    events: &'a dyn FrontendEventPublisher,
}

impl<'a> QueueMutationPorts<'a> {
    pub(crate) fn new(
        queue_repository: &'a dyn QueueRepository,
        events: &'a dyn FrontendEventPublisher,
    ) -> Self {
        Self {
            queue_repository,
            events,
        }
    }

    async fn repo_remove_task(&self, task_id: &str) -> AppResult<()> {
        self.queue_repository.remove_task(task_id).await
    }

    async fn repo_reorder_tasks(&self, task_ids: Vec<String>) -> AppResult<()> {
        self.queue_repository.reorder_tasks(task_ids).await
    }

    async fn repo_update_save_name(
        &self,
        task_id: &str,
        save_name: Option<String>,
    ) -> AppResult<()> {
        self.queue_repository
            .update_save_name(task_id, save_name)
            .await
    }

    async fn repo_pause_queue(&self) -> AppResult<()> {
        self.queue_repository
            .set_run_status(QueueRunStatus::Paused)
            .await
    }

    fn mark_queue_paused(&self) {
        self.events.queue_state_changed();
    }

    fn mark_task_removed(&self) {
        self.events.queue_state_changed();
    }

    fn mark_tasks_reordered(&self) {
        self.events.queue_state_changed();
    }

    async fn pause_queue(&self) -> AppResult<()> {
        self.repo_pause_queue().await?;
        self.mark_queue_paused();
        Ok(())
    }

    pub(crate) async fn handle_queue_pause(&self) -> AppResult<()> {
        self.pause_queue().await
    }

    async fn remove_task(&self, task_id: &str) -> AppResult<()> {
        self.repo_remove_task(task_id).await?;
        self.mark_task_removed();
        Ok(())
    }

    pub(crate) async fn handle_task_removal(&self, task_id: &str) -> AppResult<()> {
        self.remove_task(task_id).await
    }

    async fn update_save_name(&self, task_id: &str, save_name: Option<String>) -> AppResult<()> {
        self.repo_update_save_name(task_id, save_name).await?;
        self.events.queue_state_changed();
        Ok(())
    }

    pub(crate) async fn handle_save_name_update(
        &self,
        task_id: &str,
        save_name: Option<String>,
    ) -> AppResult<()> {
        self.update_save_name(task_id, save_name).await
    }

    async fn reorder_tasks(&self, task_ids: Vec<String>) -> AppResult<()> {
        self.repo_reorder_tasks(task_ids).await?;
        self.mark_tasks_reordered();
        Ok(())
    }

    pub(crate) async fn handle_tasks_reorder(&self, task_ids: Vec<String>) -> AppResult<()> {
        self.reorder_tasks(task_ids).await
    }
}
