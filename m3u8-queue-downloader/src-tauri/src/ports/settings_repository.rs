use crate::application::app_error::AppResult;
use crate::application::settings::AppSettings;
use std::sync::Arc;

pub(crate) trait SettingsRepository: Send + Sync {
    fn get(&self) -> AppSettings;
    fn update(&self, settings: AppSettings) -> AppResult<AppSettings>;
}

impl<T> SettingsRepository for Arc<T>
where
    T: SettingsRepository + ?Sized,
{
    fn get(&self) -> AppSettings {
        self.as_ref().get()
    }

    fn update(&self, settings: AppSettings) -> AppResult<AppSettings> {
        self.as_ref().update(settings)
    }
}
