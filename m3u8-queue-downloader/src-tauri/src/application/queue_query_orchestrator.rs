use crate::application::queue_state_snapshot::QueueStateSnapshot;
use crate::application::QueueRepository;

pub(crate) struct QueueQueryPorts<'a> {
    queue_repository: &'a dyn QueueRepository,
}

impl<'a> QueueQueryPorts<'a> {
    pub(crate) fn new(queue_repository: &'a dyn QueueRepository) -> Self {
        Self { queue_repository }
    }

    pub(crate) async fn get_state_snapshot(&self) -> QueueStateSnapshot {
        self.queue_repository.get_state_snapshot().await
    }
}
