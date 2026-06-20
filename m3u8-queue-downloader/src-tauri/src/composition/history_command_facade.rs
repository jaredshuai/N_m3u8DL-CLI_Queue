use crate::application::app_error::AppResult;
use crate::application::history_use_cases::HistoryUseCases;
use crate::composition::dependency_graph::DependencyGraph;
use crate::domain::history::HistoryStatus;

pub(crate) struct HistoryCommandFacade {
    dependencies: DependencyGraph,
}

impl HistoryCommandFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    pub(crate) fn remove_history_task(
        &self,
        status: HistoryStatus,
        task_id: &str,
    ) -> AppResult<()> {
        let ports = self.dependencies.history_orchestrator();
        HistoryUseCases::new(ports).remove_task(status, task_id)
    }
}
