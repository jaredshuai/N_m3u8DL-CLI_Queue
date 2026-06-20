use crate::application::app_error::AppResult;
use crate::application::history_repository_outcomes::{HistoryFindOutcome, HistoryRemoveOutcome};
use crate::application::history_task_page::HistoryTaskPage;
use crate::application::task_snapshot::TaskSnapshot;
use crate::domain::history::HistoryStatus;
use std::sync::Arc;

pub(crate) trait HistoryRepository: Send + Sync {
    fn append(&self, task: &TaskSnapshot) -> AppResult<()>;
    fn get_page(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> AppResult<HistoryTaskPage>;
    fn find_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<HistoryFindOutcome>;
    fn remove_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<HistoryRemoveOutcome>;
}

impl<T> HistoryRepository for Arc<T>
where
    T: HistoryRepository + ?Sized,
{
    fn append(&self, task: &TaskSnapshot) -> AppResult<()> {
        self.as_ref().append(task)
    }

    fn get_page(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> AppResult<HistoryTaskPage> {
        self.as_ref().get_page(status, offset, limit)
    }

    fn find_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<HistoryFindOutcome> {
        self.as_ref().find_task(status, task_id)
    }

    fn remove_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<HistoryRemoveOutcome> {
        self.as_ref().remove_task(status, task_id)
    }
}
