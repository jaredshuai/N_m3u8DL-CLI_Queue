use crate::models::AppSettings;
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
    std::fs::write(path, json).map_err(|e| e.to_string())
}
