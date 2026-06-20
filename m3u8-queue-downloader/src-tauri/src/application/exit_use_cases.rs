use crate::application::app_error::AppResult;
use crate::application::close_policy::{resolve_close_action, CloseAction, CloseRequestSource};
use crate::application::exit_orchestrator::ExitPorts;

pub(crate) struct ExitUseCases<'a> {
    ports: ExitPorts<'a>,
}

impl<'a> ExitUseCases<'a> {
    pub(crate) fn new(ports: ExitPorts<'a>) -> Self {
        Self { ports }
    }

    pub(crate) async fn request_close(&self, source: CloseRequestSource) -> AppResult<()> {
        match resolve_close_action(self.ports.close_button_behavior(), source) {
            CloseAction::HideToTray => self.ports.hide_main_window(),
            CloseAction::ExitApplication => self.ports.exit_application().await,
        }
    }

    pub(crate) fn cancel_auto_shutdown(&self) -> AppResult<()> {
        self.ports.cancel_auto_shutdown()
    }
}
