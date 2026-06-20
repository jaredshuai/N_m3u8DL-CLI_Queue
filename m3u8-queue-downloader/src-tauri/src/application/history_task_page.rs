use crate::application::task_snapshot::TaskSnapshot;

#[derive(Debug, Clone)]
pub(crate) struct HistoryTaskPage {
    pub tasks: Vec<TaskSnapshot>,
    pub has_more: bool,
    pub next_offset: usize,
}
