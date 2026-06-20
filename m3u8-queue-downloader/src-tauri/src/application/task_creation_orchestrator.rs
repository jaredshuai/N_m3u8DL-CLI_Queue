use crate::application::queue_requests::AddTaskPayload;
use crate::application::task_snapshot::TaskSnapshot;
use crate::application::Clock;
use crate::application::TaskIdGenerator;
use crate::domain::task::Task;
use chrono::{DateTime, Utc};

#[derive(Clone, Copy)]
pub(crate) struct TaskCreationPorts<'a> {
    task_id_generator: &'a dyn TaskIdGenerator,
    clock: &'a dyn Clock,
}

impl<'a> TaskCreationPorts<'a> {
    pub(crate) fn new(task_id_generator: &'a dyn TaskIdGenerator, clock: &'a dyn Clock) -> Self {
        Self {
            task_id_generator,
            clock,
        }
    }

    pub(crate) fn create_queued_task_from_payload(&self, payload: AddTaskPayload) -> Task {
        Task::new_queued(
            self.next_task_id(),
            payload.url,
            payload.save_name,
            payload.headers,
            self.now(),
        )
    }

    pub(crate) fn create_queued_task_from_history_retry(
        &self,
        history_task: &TaskSnapshot,
    ) -> Task {
        Task::new_queued(
            self.next_task_id(),
            history_task.url.clone(),
            history_task.save_name.clone(),
            history_task.headers.clone(),
            self.now(),
        )
    }

    fn next_task_id(&self) -> String {
        self.task_id_generator.next_task_id()
    }

    fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::Clock;
    use crate::application::TaskIdGenerator;
    use chrono::{DateTime, Utc};

    struct FixedTaskIdGenerator;

    impl TaskIdGenerator for FixedTaskIdGenerator {
        fn next_task_id(&self) -> String {
            "task-new".to_string()
        }
    }

    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            DateTime::from_timestamp(42, 0).expect("valid timestamp")
        }
    }

    #[test]
    fn creates_queued_task_from_add_payload() {
        let ports = TaskCreationPorts::new(&FixedTaskIdGenerator, &FixedClock);

        let task = ports.create_queued_task_from_payload(AddTaskPayload {
            url: "https://example.com/video.m3u8".to_string(),
            save_name: Some("video".to_string()),
            headers: Some("User-Agent: test".to_string()),
        });

        assert_eq!(task.id, "task-new");
        assert_eq!(task.url, "https://example.com/video.m3u8");
        assert_eq!(task.save_name.as_deref(), Some("video"));
        assert_eq!(task.headers.as_deref(), Some("User-Agent: test"));
        assert_eq!(task.created_at, FixedClock.now());
    }

    #[test]
    fn creates_queued_task_from_failed_history_retry() {
        let ports = TaskCreationPorts::new(&FixedTaskIdGenerator, &FixedClock);
        let history_task = Task::new_queued(
            "old-task".to_string(),
            "https://example.com/old.m3u8".to_string(),
            Some("old".to_string()),
            Some("Cookie: old".to_string()),
            DateTime::from_timestamp(1, 0).expect("valid timestamp"),
        );
        let history_task = TaskSnapshot::from(&history_task);

        let task = ports.create_queued_task_from_history_retry(&history_task);

        assert_eq!(task.id, "task-new");
        assert_eq!(task.url, "https://example.com/old.m3u8");
        assert_eq!(task.save_name.as_deref(), Some("old"));
        assert_eq!(task.headers.as_deref(), Some("Cookie: old"));
        assert_eq!(task.created_at, FixedClock.now());
    }
}
