use crate::application::app_error::AppResult;
use crate::application::history_use_cases::HistoryUseCases;
use crate::application::query_models::{
    CliOutputPage, CliTerminalState, HistoryPage, QueueStateView,
};
use crate::application::settings::AppSettings;
use crate::application::terminal_output_use_cases::TerminalOutputUseCases;
use crate::composition::dependency_graph::DependencyGraph;
use crate::composition::pending_history_facade::PendingHistoryFacade;
use crate::domain::history::HistoryStatus;

pub(crate) struct ReadModelQueryFacade {
    dependencies: DependencyGraph,
}

impl ReadModelQueryFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    pub(crate) async fn get_queue_state(&self) -> QueueStateView {
        let ports = self.dependencies.queue_query_orchestrator();
        ports.get_state_snapshot().await.into()
    }

    pub(crate) fn get_app_settings(&self) -> AppSettings {
        let ports = self.dependencies.settings_query_orchestrator();
        ports.get()
    }

    pub(crate) async fn history_page_query(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> impl FnOnce() -> AppResult<HistoryPage> + Send + 'static {
        let pending_history = PendingHistoryFacade::new(self.dependencies.clone());
        if let Err(err) = pending_history.flush_pending_history_tasks().await {
            self.dependencies.diagnostics.warn(&format!(
                "Failed to flush pending history before reading history page: {err}"
            ));
        }

        let ports = self.dependencies.history_orchestrator();
        move || HistoryUseCases::new(ports).page(status, offset, limit)
    }

    pub(crate) fn cli_output_tail_query(
        &self,
        task_id: String,
        limit: usize,
    ) -> impl FnOnce() -> AppResult<CliOutputPage> + Send + 'static {
        let ports = self.dependencies.terminal_output_orchestrator();
        move || TerminalOutputUseCases::new(ports).tail(&task_id, limit)
    }

    pub(crate) fn cli_output_page_query(
        &self,
        task_id: String,
        offset: usize,
        limit: usize,
    ) -> impl FnOnce() -> AppResult<CliOutputPage> + Send + 'static {
        let ports = self.dependencies.terminal_output_orchestrator();
        move || TerminalOutputUseCases::new(ports).page(&task_id, offset, limit)
    }

    pub(crate) fn cli_terminal_state_query(
        &self,
        task_id: String,
        limit: usize,
    ) -> impl FnOnce() -> AppResult<CliTerminalState> + Send + 'static {
        let ports = self.dependencies.terminal_output_orchestrator();
        move || TerminalOutputUseCases::new(ports).terminal_state(&task_id, limit)
    }
}
