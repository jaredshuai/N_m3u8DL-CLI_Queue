use crate::application::app_error::AppResult;
use crate::application::history_repository_outcomes::HistoryRemoveOutcome;
use crate::application::history_task_page::HistoryTaskPage;
use crate::application::HistoryRepository;
use crate::domain::history::HistoryStatus;
use std::sync::Arc;

pub(crate) struct HistoryPorts {
    history_repository: Arc<dyn HistoryRepository>,
}

impl HistoryPorts {
    pub(crate) fn new(history_repository: Arc<dyn HistoryRepository>) -> Self {
        Self { history_repository }
    }

    pub(crate) fn page(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> AppResult<HistoryTaskPage> {
        self.history_repository.get_page(status, offset, limit)
    }

    pub(crate) fn remove_task(
        &self,
        status: HistoryStatus,
        task_id: &str,
    ) -> AppResult<HistoryRemoveOutcome> {
        self.history_repository.remove_task(status, task_id)
    }
}
