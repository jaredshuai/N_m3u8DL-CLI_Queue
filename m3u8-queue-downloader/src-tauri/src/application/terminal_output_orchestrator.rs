use crate::application::app_error::AppResult;
use crate::application::terminal_output_outcomes::TerminalActiveLine;
use crate::application::terminal_output_page::TerminalOutputPage;
use crate::application::TerminalOutputRepository;
use std::sync::Arc;

pub(crate) struct TerminalOutputPorts {
    terminal_output_repository: Arc<dyn TerminalOutputRepository>,
}

impl TerminalOutputPorts {
    pub(crate) fn new(terminal_output_repository: Arc<dyn TerminalOutputRepository>) -> Self {
        Self {
            terminal_output_repository,
        }
    }

    pub(crate) fn tail(&self, task_id: &str, limit: usize) -> AppResult<TerminalOutputPage> {
        self.terminal_output_repository.tail(task_id, limit)
    }

    pub(crate) fn page(
        &self,
        task_id: &str,
        offset: usize,
        limit: usize,
    ) -> AppResult<TerminalOutputPage> {
        self.terminal_output_repository.page(task_id, offset, limit)
    }

    pub(crate) fn active_line(&self, task_id: &str) -> TerminalActiveLine {
        self.terminal_output_repository.get_active_line(task_id)
    }
}
