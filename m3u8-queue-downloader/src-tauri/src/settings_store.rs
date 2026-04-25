use crate::app_error::AppResult;
use crate::download_dir::normalize_download_dir;
use crate::models::AppSettings;
use crate::persistence::write_atomic;
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

fn load_settings(path: &Path) -> Option<AppSettings> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_settings(settings: &AppSettings, path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    write_atomic(path, json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CloseButtonBehavior;
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
            })
            .expect("save settings");

        assert_eq!(updated.download_dir, None);
        assert_eq!(store.get().download_dir, None);

        let _ = std::fs::remove_file(path);
    }
}
