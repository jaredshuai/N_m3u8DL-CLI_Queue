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

    pub fn update(&self, settings: AppSettings) -> Result<AppSettings, String> {
        *self.state.lock().expect("settings mutex poisoned") = settings.clone();
        save_settings(&settings, &self.path)?;
        Ok(settings)
    }
}

fn load_settings(path: &Path) -> Option<AppSettings> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_settings(settings: &AppSettings, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
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

        assert_eq!(store.get().close_button_behavior, CloseButtonBehavior::CloseToTray);
        assert!(!store.get().auto_action_on_complete);

        store
            .update(AppSettings {
                close_button_behavior: CloseButtonBehavior::Exit,
                auto_action_on_complete: true,
            })
            .expect("save settings");

        let reloaded = SettingsStore::new(path.clone());
        assert_eq!(reloaded.get().close_button_behavior, CloseButtonBehavior::Exit);
        assert!(reloaded.get().auto_action_on_complete);

        let _ = std::fs::remove_file(path);
    }
}
