use crate::application::app_error::AppResult;
use crate::application::settings::{AppSettings, ThemePreference};
use crate::composition::dependency_graph::DependencyGraph;
use crate::ports::event_publisher::FrontendEventPublisher;

pub(crate) struct SettingsCommandFacade {
    dependencies: DependencyGraph,
}

impl SettingsCommandFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    pub(crate) fn update_app_settings(
        &self,
        events: &dyn FrontendEventPublisher,
        settings: AppSettings,
    ) -> AppResult<AppSettings> {
        let ports = self.dependencies.settings_orchestrator(events);
        ports.update_settings_and_handle_auto_action_change(settings)
    }

    pub(crate) fn update_theme(
        &self,
        events: &dyn FrontendEventPublisher,
        theme: ThemePreference,
    ) -> AppResult<AppSettings> {
        let ports = self.dependencies.settings_orchestrator(events);
        ports.update_theme(theme)
    }
}
