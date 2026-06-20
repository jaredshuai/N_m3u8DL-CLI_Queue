use crate::application::task_snapshot::TaskSnapshot;

pub(crate) enum HistoryFindOutcome {
    Found(TaskSnapshot),
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryRemoveOutcome {
    Removed,
    Missing,
}
