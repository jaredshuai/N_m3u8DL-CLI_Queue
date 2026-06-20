use crate::application::app_error::AppResult;
use crate::application::terminal_output_outcomes::TerminalActiveLine;
use crate::application::terminal_output_page::TerminalOutputPage;
use std::sync::Arc;

pub(crate) trait TerminalOutputRepository: Send + Sync {
    fn append_line(&self, task_id: &str, line: &str) -> AppResult<()>;
    fn page(&self, task_id: &str, offset: usize, limit: usize) -> AppResult<TerminalOutputPage>;
    fn tail(&self, task_id: &str, limit: usize) -> AppResult<TerminalOutputPage>;
    fn set_active_line(&self, task_id: &str, line: String);
    fn clear_active_line(&self, task_id: &str);
    fn get_active_line(&self, task_id: &str) -> TerminalActiveLine;
}

impl<T> TerminalOutputRepository for Arc<T>
where
    T: TerminalOutputRepository + ?Sized,
{
    fn append_line(&self, task_id: &str, line: &str) -> AppResult<()> {
        self.as_ref().append_line(task_id, line)
    }

    fn page(&self, task_id: &str, offset: usize, limit: usize) -> AppResult<TerminalOutputPage> {
        self.as_ref().page(task_id, offset, limit)
    }

    fn tail(&self, task_id: &str, limit: usize) -> AppResult<TerminalOutputPage> {
        self.as_ref().tail(task_id, limit)
    }

    fn set_active_line(&self, task_id: &str, line: String) {
        self.as_ref().set_active_line(task_id, line);
    }

    fn clear_active_line(&self, task_id: &str) {
        self.as_ref().clear_active_line(task_id);
    }

    fn get_active_line(&self, task_id: &str) -> TerminalActiveLine {
        self.as_ref().get_active_line(task_id)
    }
}
