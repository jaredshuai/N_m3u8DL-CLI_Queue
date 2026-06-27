use crate::adapters::settings_dto::AppSettingsDto;
use crate::adapters::storage_files;
use crate::application::app_error::AppResult;
use crate::application::settings::{normalize_download_dir, AppSettings};
use crate::ports::settings_repository::SettingsRepository;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct SettingsStore {
    path: PathBuf,
    state: Mutex<AppSettings>,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        let state = load_settings(&path).unwrap_or_default();
        Self {
            path,
            state: Mutex::new(state),
        }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("m3u8-queue-downloader")
            .join("settings.json")
    }

    pub fn get(&self) -> AppSettings {
        self.state.lock().expect("settings mutex poisoned").clone()
    }

    pub fn update(&self, settings: AppSettings) -> AppResult<AppSettings> {
        let normalized = AppSettings {
            download_dir: normalize_download_dir(settings.download_dir),
            ..settings
        };
        save_settings(&normalized, &self.path)?;
        *self.state.lock().expect("settings mutex poisoned") = normalized.clone();
        Ok(normalized)
    }
}

impl SettingsRepository for SettingsStore {
    fn get(&self) -> AppSettings {
        SettingsStore::get(self)
    }

    fn update(&self, settings: AppSettings) -> AppResult<AppSettings> {
        SettingsStore::update(self, settings)
    }
}

fn load_settings(path: &Path) -> Option<AppSettings> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let settings: AppSettingsDto = serde_json::from_str(&content).ok()?;
    Some(settings.into())
}

fn save_settings(settings: &AppSettings, path: &Path) -> AppResult<()> {
    let settings = AppSettingsDto::from(settings);
    let json = serde_json::to_string_pretty(&settings)?;
    storage_files::write_atomic(path, json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::settings::CloseButtonBehavior;
    use uuid::Uuid;

    #[test]
    fn settings_round_trip() {
        let path = std::env::temp_dir().join(format!("settings-{}.json", Uuid::new_v4()));
        let store = SettingsStore::new(path.clone());

        assert_eq!(
            store.get().close_button_behavior,
            CloseButtonBehavior::CloseToTray
        );
        assert!(!store.get().auto_action_on_complete);
        assert_eq!(store.get().download_dir, None);

        store
            .update(AppSettings {
                close_button_behavior: CloseButtonBehavior::Exit,
                auto_action_on_complete: true,
                download_dir: Some("D:/Videos".to_string()),
                theme: crate::application::settings::ThemePreference::Dark,
            })
            .expect("save settings");

        let reloaded = SettingsStore::new(path.clone());
        assert_eq!(
            reloaded.get().close_button_behavior,
            CloseButtonBehavior::Exit
        );
        assert!(reloaded.get().auto_action_on_complete);
        assert_eq!(reloaded.get().download_dir.as_deref(), Some("D:/Videos"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_save_does_not_mutate_in_memory_state() {
        let path = std::env::temp_dir().join(format!("settings-dir-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create blocking directory");
        let store = SettingsStore::new(path.clone());
        let original = store.get();

        let result = store.update(AppSettings {
            close_button_behavior: CloseButtonBehavior::Exit,
            auto_action_on_complete: true,
            download_dir: Some("D:/Blocked".to_string()),
            theme: crate::application::settings::ThemePreference::Light,
        });

        assert!(result.is_err());
        assert_eq!(store.get(), original);

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn update_normalizes_empty_download_dir_to_none() {
        let path = std::env::temp_dir().join(format!("settings-{}.json", Uuid::new_v4()));
        let store = SettingsStore::new(path.clone());

        let updated = store
            .update(AppSettings {
                close_button_behavior: CloseButtonBehavior::CloseToTray,
                auto_action_on_complete: false,
                download_dir: Some("   ".to_string()),
                theme: crate::application::settings::ThemePreference::Auto,
            })
            .expect("save settings");

        assert_eq!(updated.download_dir, None);
        assert_eq!(store.get().download_dir, None);

        let _ = std::fs::remove_file(path);
    }
}
