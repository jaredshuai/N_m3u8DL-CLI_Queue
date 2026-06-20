use crate::application::app_error::{AppError, AppResult};
use crate::application::history_orchestrator::HistoryPorts;
use crate::application::history_repository_outcomes::HistoryRemoveOutcome;
use crate::application::query_models::{HistoryPage, TaskView};
use crate::domain::history::HistoryStatus;

pub(crate) struct HistoryUseCases {
    ports: HistoryPorts,
}

impl HistoryUseCases {
    pub(crate) fn new(ports: HistoryPorts) -> Self {
        Self { ports }
    }

    pub(crate) fn page(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> AppResult<HistoryPage> {
        let page = self.ports.page(status, offset, limit)?;
        Ok(HistoryPage {
            tasks: page.tasks.into_iter().map(TaskView::from).collect(),
            has_more: page.has_more,
            next_offset: page.next_offset,
        })
    }

    pub(crate) fn remove_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<()> {
        match self.ports.remove_task(status, task_id)? {
            HistoryRemoveOutcome::Removed => Ok(()),
            HistoryRemoveOutcome::Missing => Err(AppError::message(format!(
                "History task {task_id} not found"
            ))),
        }
    }
}
