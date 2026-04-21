use crate::models::{QueueState, TaskStatus};
use std::path::{Path, PathBuf};

/// Handles saving and loading queue state to/from a JSON file
pub struct Persistence;

impl Persistence {
    /// Returns the default file path for queue state persistence
    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("m3u8-queue-downloader")
            .join("queue_state.json")
    }

    /// Save queue state to a JSON file
    pub fn save(state: &QueueState, path: &Path) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Load queue state from a JSON file. Returns None if file doesn't exist.
    pub fn load(path: &Path) -> Option<QueueState> {
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(path).ok()?;
        let mut state: QueueState = serde_json::from_str(&content).ok()?;
        // Reset any Downloading tasks to Waiting since the CLI process is gone
        for task in &mut state.tasks {
            if task.status == TaskStatus::Downloading {
                task.status = TaskStatus::Waiting;
            }
        }
        // Clear current_task_id since no process is running after restart
        state.current_task_id = None;
        Some(state)
    }
}
