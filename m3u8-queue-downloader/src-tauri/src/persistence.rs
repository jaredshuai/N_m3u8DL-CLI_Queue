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
        write_atomic(path, json.as_bytes())
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

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| "missing file name for atomic write".to_string())?
        .to_string_lossy();
    let tmp_path = path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()));

    std::fs::write(&tmp_path, bytes).map_err(|e| e.to_string())?;
    replace_file_atomically(&tmp_path, path)
}

#[cfg(target_os = "windows")]
fn replace_file_atomically(tmp_path: &Path, path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    if !path.exists() {
        return std::fs::rename(tmp_path, path).map_err(|e| e.to_string());
    }

    let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let tmp_wide: Vec<u16> = tmp_path.as_os_str().encode_wide().chain(Some(0)).collect();
    let replaced = unsafe {
        ReplaceFileW(
            path_wide.as_ptr(),
            tmp_wide.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };

    if replaced == 0 {
        let err = std::io::Error::last_os_error().to_string();
        let _ = std::fs::remove_file(tmp_path);
        return Err(err);
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_file_atomically(tmp_path: &Path, path: &Path) -> Result<(), String> {
    std::fs::rename(tmp_path, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{QueueState, Task, TaskStatus};
    use chrono::Utc;
    use std::collections::VecDeque;
    use uuid::Uuid;

    fn temp_state_path() -> PathBuf {
        std::env::temp_dir().join(format!("queue-state-{}.json", Uuid::new_v4()))
    }

    fn sample_state() -> QueueState {
        QueueState {
            tasks: vec![Task {
                id: "task-1".to_string(),
                url: "https://example.com/test.m3u8".to_string(),
                save_name: Some("sample".to_string()),
                headers: None,
                status: TaskStatus::Downloading,
                retry_count: 0,
                progress: 0.42,
                speed: "1 MB/s".to_string(),
                threads: "8".to_string(),
                output_path: None,
                error_message: None,
                created_at: Utc::now(),
                log_lines: VecDeque::new(),
            }],
            current_task_id: Some("task-1".to_string()),
            is_running: true,
        }
    }

    #[test]
    fn save_and_load_round_trip_with_restart_normalization() {
        let path = temp_state_path();
        Persistence::save(&sample_state(), &path).expect("save queue state");

        let loaded = Persistence::load(&path).expect("load queue state");
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].status, TaskStatus::Waiting);
        assert!(loaded.current_task_id.is_none());

        std::fs::remove_file(path).expect("cleanup queue state");
    }
}
