use crate::application::app_error::AppResult;
use crate::application::process_runner_outcomes::TaskTerminationClaimOutcome;
use crate::application::query_models::TaskView;
use crate::application::queue_requests::AddTaskPayload;
use crate::composition::dependency_graph::DependencyGraph;
use crate::ports::event_publisher::FrontendEventPublisher;

pub(crate) struct QueueCommandFacade {
    dependencies: DependencyGraph,
}

impl QueueCommandFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    pub(crate) async fn add_task(
        &self,
        events: &dyn FrontendEventPublisher,
        payload: AddTaskPayload,
    ) -> AppResult<TaskView> {
        let process_runner = self.dependencies.create_task_process_runner();
        let scheduling_ports = self
            .dependencies
            .queue_scheduling_orchestrator(events, process_runner.as_ref());
        let task_creation_orchestrator = self.dependencies.task_creation_orchestrator();
        scheduling_ports
            .handle_queue_add(task_creation_orchestrator, payload)
            .await
            .map(TaskView::from)
    }

    pub(crate) async fn remove_task(
        &self,
        events: &dyn FrontendEventPublisher,
        task_id: &str,
    ) -> AppResult<()> {
        let ports = self.dependencies.queue_mutation_orchestrator(events);
        ports.handle_task_removal(task_id).await
    }

    pub(crate) async fn update_save_name(
        &self,
        events: &dyn FrontendEventPublisher,
        task_id: &str,
        save_name: Option<String>,
    ) -> AppResult<()> {
        let ports = self.dependencies.queue_mutation_orchestrator(events);
        ports.handle_save_name_update(task_id, save_name).await
    }

    pub(crate) async fn retry_task(
        &self,
        events: &dyn FrontendEventPublisher,
        task_id: &str,
    ) -> AppResult<TaskView> {
        let process_runner = self.dependencies.create_task_process_runner();
        let scheduling_ports = self
            .dependencies
            .queue_scheduling_orchestrator(events, process_runner.as_ref());
        let task_creation_orchestrator = self.dependencies.task_creation_orchestrator();
        scheduling_ports
            .handle_queue_retry(task_id, task_creation_orchestrator)
            .await
            .map(TaskView::from)
    }

    pub(crate) async fn reorder_tasks(
        &self,
        events: &dyn FrontendEventPublisher,
        task_ids: Vec<String>,
    ) -> AppResult<()> {
        let ports = self.dependencies.queue_mutation_orchestrator(events);
        ports.handle_tasks_reorder(task_ids).await
    }

    pub(crate) async fn start_queue(&self, events: &dyn FrontendEventPublisher) -> AppResult<()> {
        let process_runner = self.dependencies.create_task_process_runner();
        let ports = self
            .dependencies
            .queue_scheduling_orchestrator(events, process_runner.as_ref());
        ports.handle_queue_start().await
    }

    pub(crate) async fn pause_queue(&self, events: &dyn FrontendEventPublisher) -> AppResult<()> {
        let ports = self.dependencies.queue_mutation_orchestrator(events);
        ports.handle_queue_pause().await
    }

    /// Claim the exact child generation before changing queue state. A waiter
    /// that exits after the claim pauses until this method commits or aborts,
    /// so persistence failure restores the natural Completed/Failed event.
    pub(crate) async fn stop_task(&self, task_id: &str) -> AppResult<()> {
        let (queue_repository, process_supervisor) = self.dependencies.stop_task_ports();
        let claim = match process_supervisor.claim_task_termination(task_id).await? {
            TaskTerminationClaimOutcome::Claimed(claim) => claim,
            TaskTerminationClaimOutcome::AlreadyClaimed => return Ok(()),
            TaskTerminationClaimOutcome::AlreadyExited => return Ok(()),
        };

        if let Err(err) = queue_repository.stop_task(task_id).await {
            process_supervisor.abort_task_termination(&claim).await;
            return Err(err);
        }

        process_supervisor.terminate_claimed_task(&claim).await
    }
}
