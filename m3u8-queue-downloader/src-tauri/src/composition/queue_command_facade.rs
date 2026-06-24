use crate::application::app_error::AppResult;
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

    /// Stop a running (downloading) task: mark it Cancelled on the queue side
/// first (so any Cancelled lifecycle event finds the queue already in the
/// desired state), then kill the live child process.
///
/// Order rationale (resolves the cubic P1 race between "mark-before-kill"
/// and "kill-before-mark"): `terminate_task` is always-OK, so there is no
/// error path where the queue is marked Cancelled but the kill failed and
/// returned an error — the kill is best-effort inside `terminate_task`.
/// Combined with mark-first ordering, both races are closed:
///   - The Cancelled event always finds the queue in the desired state
///     (no stale-state race — cubic round 3 P1).
///   - No error is ever returned after the mark, so there's no
///     "queue Cancelled but process still running, error surfaced"
///     inconsistency (cubic round 2 P1).
/// See ADR-0009.
pub(crate) async fn stop_task(&self, task_id: &str) -> AppResult<()> {
    let (queue_repository, process_supervisor) = self.dependencies.stop_task_ports();
    // 1. Mark Cancelled first — clears current_task + persists status.
    //    Done first so the eventual Cancelled lifecycle event sees the
    //    queue already in the desired state.
    queue_repository.stop_task(task_id).await?;
    // 2. Kill the process. Always-OK: sets cancelling marker + emits
    //    Cancelled event. The kill is best-effort — on failure the marker
    //    is kept so spawn_wait_task won't emit a confusing Failed event.
    process_supervisor.terminate_task(task_id).await
}
}
