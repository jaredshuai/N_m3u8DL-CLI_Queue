use crate::application::settings::AppSettings;
use crate::application::SettingsRepository;

pub(crate) struct SettingsQueryPorts<'a> {
    settings_repository: &'a dyn SettingsRepository,
}

impl<'a> SettingsQueryPorts<'a> {
    pub(crate) fn new(settings_repository: &'a dyn SettingsRepository) -> Self {
        Self {
            settings_repository,
        }
    }

    pub(crate) fn get(&self) -> AppSettings {
        self.settings_repository.get()
    }
}
