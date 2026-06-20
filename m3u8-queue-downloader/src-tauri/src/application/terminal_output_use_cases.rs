use crate::application::app_error::AppResult;
use crate::application::query_models::{CliOutputPage, CliTerminalState};
use crate::application::terminal_output_orchestrator::TerminalOutputPorts;
use crate::application::terminal_output_outcomes::TerminalActiveLine;
use crate::application::terminal_output_page::TerminalOutputPage;

pub(crate) struct TerminalOutputUseCases {
    ports: TerminalOutputPorts,
}

impl TerminalOutputUseCases {
    pub(crate) fn new(ports: TerminalOutputPorts) -> Self {
        Self { ports }
    }

    pub(crate) fn tail(&self, task_id: &str, limit: usize) -> AppResult<CliOutputPage> {
        self.ports
            .tail(task_id, limit)
            .map(cli_output_page_from_repository)
    }

    pub(crate) fn page(
        &self,
        task_id: &str,
        offset: usize,
        limit: usize,
    ) -> AppResult<CliOutputPage> {
        self.ports
            .page(task_id, offset, limit)
            .map(cli_output_page_from_repository)
    }

    pub(crate) fn terminal_state(
        &self,
        task_id: &str,
        limit: usize,
    ) -> AppResult<CliTerminalState> {
        let page = self.ports.tail(task_id, limit)?;
        let active_line = match self.ports.active_line(task_id) {
            TerminalActiveLine::Present(line) => line,
            TerminalActiveLine::Missing => String::new(),
        };
        Ok(CliTerminalState {
            committed_lines: page.lines,
            active_line,
            offset: page.offset,
            total: page.total,
            has_more_before: page.has_more_before,
        })
    }
}

fn cli_output_page_from_repository(page: TerminalOutputPage) -> CliOutputPage {
    CliOutputPage {
        lines: page.lines,
        offset: page.offset,
        total: page.total,
        next_offset: page.next_offset,
        has_more_before: page.has_more_before,
        has_more_after: page.has_more_after,
    }
}
