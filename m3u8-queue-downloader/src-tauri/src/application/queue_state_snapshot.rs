use crate::application::task_snapshot::TaskSnapshot;
use crate::domain::queue::QueueAggregate;

#[derive(Debug, Clone)]
pub(crate) struct QueueStateSnapshot {
    pub(crate) tasks: Vec<TaskSnapshot>,
    pub(crate) current_task_id: Option<String>,
    pub(crate) is_running: bool,
}

impl From<&QueueAggregate> for QueueStateSnapshot {
    fn from(state: &QueueAggregate) -> Self {
        Self {
            tasks: state.tasks().iter().map(TaskSnapshot::from).collect(),
            current_task_id: state.current_task_id().map(str::to_string),
            is_running: state.is_running(),
        }
    }
}
