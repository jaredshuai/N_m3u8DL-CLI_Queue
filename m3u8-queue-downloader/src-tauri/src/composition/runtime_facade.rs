use crate::application::task_lifecycle_orchestrator::TaskLifecyclePorts;
use crate::application::task_output_event_orchestrator::TaskOutputEventPorts;
use crate::application::task_process_events::{TaskLifecycleEvent, TaskOutputEvent};
use crate::composition::dependency_graph::DependencyGraph;
use crate::ports::event_publisher::FrontendEventPublisher;
use crate::ports::process_runner::TaskProcessRunner;

pub(crate) struct RuntimeFacade {
    dependencies: DependencyGraph,
}

impl RuntimeFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    fn task_lifecycle_orchestrator<'a>(
        &'a self,
        events: &'a dyn FrontendEventPublisher,
        process_runner: &'a dyn TaskProcessRunner,
    ) -> TaskLifecyclePorts<'a> {
        self.dependencies
            .task_lifecycle_orchestrator(events, process_runner)
    }

    fn task_output_event_orchestrator<'a>(
        &'a self,
        events: &'a dyn FrontendEventPublisher,
    ) -> TaskOutputEventPorts<'a> {
        self.dependencies.task_output_event_orchestrator(events)
    }

    pub(crate) async fn handle_task_lifecycle_event(
        &self,
        events: &dyn FrontendEventPublisher,
        event: TaskLifecycleEvent,
    ) {
        let process_runner = self.dependencies.create_task_process_runner();
        let lifecycle_ports = self.task_lifecycle_orchestrator(events, process_runner.as_ref());
        match event {
            TaskLifecycleEvent::Completed { id, output_path } => {
                lifecycle_ports
                    .handle_completed_child_exit(&id, &output_path)
                    .await;
            }
            TaskLifecycleEvent::Failed { id, error_message } => {
                lifecycle_ports
                    .handle_failed_child_exit(&id, &error_message)
                    .await;
            }
        }
    }

    pub(crate) async fn handle_task_output_event(
        &self,
        events: &dyn FrontendEventPublisher,
        event: TaskOutputEvent,
    ) {
        self.task_output_event_orchestrator(events)
            .handle_task_output_event(event)
            .await;
    }
}
