use crate::application::app_error::AppResult;
use crate::application::settings::AppSettings;
use crate::application::FrontendEventPublisher;
use crate::application::SettingsRepository;
use crate::application::ShutdownScheduler;

pub(crate) struct SettingsPorts<'a> {
    settings_repository: &'a dyn SettingsRepository,
    shutdown_scheduler: &'a dyn ShutdownScheduler,
    events: &'a dyn FrontendEventPublisher,
}

impl<'a> SettingsPorts<'a> {
    pub(crate) fn new(
        settings_repository: &'a dyn SettingsRepository,
        shutdown_scheduler: &'a dyn ShutdownScheduler,
        events: &'a dyn FrontendEventPublisher,
    ) -> Self {
        Self {
            settings_repository,
            shutdown_scheduler,
            events,
        }
    }

    pub(crate) fn update_settings_and_handle_auto_action_change(
        &self,
        settings: AppSettings,
    ) -> AppResult<AppSettings> {
        let previous = self.settings_repository.get();
        let updated = self.settings_repository.update(settings)?;

        if !previous.auto_action_on_complete && updated.auto_action_on_complete {
            self.shutdown_scheduler.clear_cancellation_after_reenable();
        }

        if previous.auto_action_on_complete && !updated.auto_action_on_complete {
            self.mark_auto_action_disabled_shutdown_countdown_cancelled();
        }

        Ok(updated)
    }

    fn mark_auto_action_disabled_shutdown_countdown_cancelled(&self) {
        let _ = self.shutdown_scheduler.cancel_countdown();
        self.events.shutdown_countdown_cancelled();
    }
}
