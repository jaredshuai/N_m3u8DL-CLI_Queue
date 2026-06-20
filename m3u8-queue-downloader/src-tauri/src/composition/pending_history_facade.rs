use crate::application::app_error::AppResult;
use crate::application::terminal_history_use_cases::{self, FlushedHistoryTask};
use crate::composition::dependency_graph::DependencyGraph;

pub(crate) struct PendingHistoryFacade {
    dependencies: DependencyGraph,
}

impl PendingHistoryFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    pub(crate) async fn flush_pending_history_tasks(&self) -> AppResult<Vec<FlushedHistoryTask>> {
        let terminal_history_orchestrator = self.dependencies.terminal_history_orchestrator();
        terminal_history_use_cases::flush_pending_history_tasks(&terminal_history_orchestrator)
            .await
    }
}
