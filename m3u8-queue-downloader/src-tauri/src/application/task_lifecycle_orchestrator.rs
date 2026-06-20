use crate::application::queue_scheduling_orchestrator::QueueSchedulingPorts;

pub(crate) struct TaskLifecyclePorts<'a> {
    scheduling_ports: QueueSchedulingPorts<'a>,
}

impl<'a> TaskLifecyclePorts<'a> {
    pub(crate) fn new(scheduling_ports: QueueSchedulingPorts<'a>) -> Self {
        Self { scheduling_ports }
    }

    pub(crate) async fn handle_completed_child_exit(&self, task_id: &str, output_path: &str) {
        self.scheduling_ports
            .handle_completed_child_exit(task_id, output_path)
            .await;
    }

    pub(crate) async fn handle_failed_child_exit(&self, task_id: &str, error_message: &str) {
        self.scheduling_ports
            .handle_failed_child_exit(task_id, error_message)
            .await;
    }
}
