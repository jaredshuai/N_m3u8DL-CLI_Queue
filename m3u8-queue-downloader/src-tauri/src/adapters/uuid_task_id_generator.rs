use crate::ports::task_id_generator::TaskIdGenerator;
use uuid::Uuid;

pub(crate) struct UuidTaskIdGenerator;

impl TaskIdGenerator for UuidTaskIdGenerator {
    fn next_task_id(&self) -> String {
        Uuid::new_v4().to_string()
    }
}
