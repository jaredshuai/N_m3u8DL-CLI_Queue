use crate::application::app_error::AppResult;
use crate::application::close_policy::CloseRequestSource;
use crate::application::exit_use_cases::ExitUseCases;
use crate::composition::dependency_graph::DependencyGraph;
use crate::ports::event_publisher::FrontendEventPublisher;

pub(crate) struct ExitCommandFacade {
    dependencies: DependencyGraph,
}

impl ExitCommandFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    pub(crate) async fn request_close(
        &self,
        events: &dyn FrontendEventPublisher,
        source: CloseRequestSource,
    ) -> AppResult<()> {
        self.exit_use_cases(events).request_close(source).await
    }

    pub(crate) fn cancel_auto_shutdown(
        &self,
        events: &dyn FrontendEventPublisher,
    ) -> AppResult<()> {
        self.exit_use_cases(events).cancel_auto_shutdown()
    }

    fn exit_use_cases<'a>(&'a self, events: &'a dyn FrontendEventPublisher) -> ExitUseCases<'a> {
        let ports = self.dependencies.exit_orchestrator(events);
        ExitUseCases::new(ports)
    }
}
