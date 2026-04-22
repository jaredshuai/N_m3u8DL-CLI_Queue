use crate::models::{HistoryPage, HistoryStatus, Task};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const HISTORY_CHUNK_SIZE: usize = 10;

#[derive(Debug, Clone)]
pub struct HistoryStore {
    base_path: PathBuf,
    append_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HistoryIndex {
    total: usize,
    chunks: Vec<HistoryChunkMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryChunkMeta {
    file: String,
    count: usize,
}

impl HistoryStore {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            append_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("m3u8-queue-downloader")
            .join("history")
    }

    pub fn append(&self, task: &Task) -> Result<(), String> {
        let _guard = self.append_lock.lock().map_err(|e| e.to_string())?;
        let status = HistoryStatus::from_task_status(&task.status)?;
        let status_dir = self.status_dir(status);
        fs::create_dir_all(&status_dir).map_err(|e| e.to_string())?;

        let mut index = self.load_reconciled_index(status)?;

        let chunk_file = if let Some(last_chunk) = index.chunks.last() {
            if last_chunk.count < HISTORY_CHUNK_SIZE {
                last_chunk.file.clone()
            } else {
                next_chunk_file_name(next_chunk_number(&index))
            }
        } else {
            next_chunk_file_name(1)
        };

        let chunk_path = status_dir.join(&chunk_file);
        let mut chunk_tasks = load_chunk(&chunk_path)?;
        chunk_tasks.push(task.clone());
        save_chunk_atomic(&chunk_path, &chunk_tasks)?;

        index = self.reconcile_index_with_disk(status, index)?;
        self.save_index_atomic(status, &index)
    }

    pub fn get_page(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> Result<HistoryPage, String> {
        if limit == 0 {
            return Ok(HistoryPage {
                tasks: Vec::new(),
                has_more: false,
                next_offset: offset,
            });
        }

        let index = self.load_reconciled_index(status)?;
        if offset >= index.total {
            return Ok(HistoryPage {
                tasks: Vec::new(),
                has_more: false,
                next_offset: offset,
            });
        }

        let mut remaining_skip = offset;
        let mut result = Vec::new();

        for chunk_meta in index.chunks.iter().rev() {
            let chunk_path = self.status_dir(status).join(&chunk_meta.file);
            let chunk_tasks = load_chunk(&chunk_path)?;

            for task in chunk_tasks.into_iter().rev() {
                if remaining_skip > 0 {
                    remaining_skip -= 1;
                    continue;
                }

                result.push(task);
                if result.len() >= limit {
                    let next_offset = offset + result.len();
                    return Ok(HistoryPage {
                        has_more: next_offset < index.total,
                        next_offset,
                        tasks: result,
                    });
                }
            }
        }

        let next_offset = offset + result.len();
        Ok(HistoryPage {
            has_more: next_offset < index.total,
            next_offset,
            tasks: result,
        })
    }

    pub fn find_task(&self, status: HistoryStatus, task_id: &str) -> Result<Option<Task>, String> {
        let index = self.load_reconciled_index(status)?;

        for chunk_meta in index.chunks.iter().rev() {
            let chunk_path = self.status_dir(status).join(&chunk_meta.file);
            let chunk_tasks = load_chunk(&chunk_path)?;
            if let Some(task) = chunk_tasks.into_iter().find(|task| task.id == task_id) {
                return Ok(Some(task));
            }
        }

        Ok(None)
    }

    fn status_dir(&self, status: HistoryStatus) -> PathBuf {
        self.base_path.join(status.as_str())
    }

    fn index_path(&self, status: HistoryStatus) -> PathBuf {
        self.status_dir(status).join("index.json")
    }

    fn load_reconciled_index(&self, status: HistoryStatus) -> Result<HistoryIndex, String> {
        let raw = self.load_index(status)?;
        self.reconcile_index_with_disk(status, raw)
    }

    fn load_index(&self, status: HistoryStatus) -> Result<HistoryIndex, String> {
        let index_path = self.index_path(status);
        if !index_path.exists() {
            return Ok(HistoryIndex::default());
        }

        let content = fs::read_to_string(index_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    fn reconcile_index_with_disk(
        &self,
        status: HistoryStatus,
        mut index: HistoryIndex,
    ) -> Result<HistoryIndex, String> {
        let status_dir = self.status_dir(status);
        let mut reconciled = BTreeMap::new();

        for chunk in index.chunks.drain(..) {
            let chunk_path = status_dir.join(&chunk.file);
            if !chunk_path.exists() {
                continue;
            }

            let actual_count = load_chunk(&chunk_path)?.len();
            if actual_count > 0 {
                reconciled.insert(chunk.file, actual_count);
            }
        }

        if status_dir.exists() {
            for entry in fs::read_dir(&status_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                    continue;
                }

                let file_name = entry.file_name();
                let file_name = file_name.to_string_lossy().to_string();
                if file_name == "index.json" || !is_chunk_file(&file_name) {
                    continue;
                }
                if reconciled.contains_key(&file_name) {
                    continue;
                }

                let actual_count = load_chunk(&entry.path())?.len();
                if actual_count > 0 {
                    reconciled.insert(file_name, actual_count);
                }
            }
        }

        let chunks: Vec<HistoryChunkMeta> = reconciled
            .into_iter()
            .map(|(file, count)| HistoryChunkMeta { file, count })
            .collect();
        let total = chunks.iter().map(|chunk| chunk.count).sum();

        Ok(HistoryIndex { total, chunks })
    }

    fn save_index_atomic(&self, status: HistoryStatus, index: &HistoryIndex) -> Result<(), String> {
        let path = self.index_path(status);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let content = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
        write_atomic(&path, content.as_bytes())
    }
}

fn next_chunk_file_name(chunk_number: usize) -> String {
    format!("{chunk_number:06}.json")
}

fn next_chunk_number(index: &HistoryIndex) -> usize {
    index
        .chunks
        .last()
        .and_then(|chunk| parse_chunk_number(&chunk.file))
        .map(|value| value + 1)
        .unwrap_or(1)
}

fn parse_chunk_number(file_name: &str) -> Option<usize> {
    file_name
        .strip_suffix(".json")
        .and_then(|value| value.parse::<usize>().ok())
}

fn is_chunk_file(file_name: &str) -> bool {
    parse_chunk_number(file_name).is_some()
}

fn load_chunk(path: &Path) -> Result<Vec<Task>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn save_chunk_atomic(path: &Path, tasks: &[Task]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(tasks).map_err(|e| e.to_string())?;
    write_atomic(path, content.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| "missing file name for atomic write".to_string())?
        .to_string_lossy();
    let tmp_path = path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()));

    fs::write(&tmp_path, bytes).map_err(|e| e.to_string())?;
    replace_file_atomically(&tmp_path, path)
}

#[cfg(target_os = "windows")]
fn replace_file_atomically(tmp_path: &Path, path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    if !path.exists() {
        return fs::rename(tmp_path, path).map_err(|e| e.to_string());
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
        let _ = fs::remove_file(tmp_path);
        return Err(err);
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_file_atomically(tmp_path: &Path, path: &Path) -> Result<(), String> {
    fs::rename(tmp_path, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TaskStatus;
    use chrono::Utc;
    use std::collections::VecDeque;
    use uuid::Uuid;

    fn temp_history_path() -> PathBuf {
        std::env::temp_dir().join(format!("history-store-{}", Uuid::new_v4()))
    }

    fn build_task(index: usize, status: TaskStatus) -> Task {
        Task {
            id: format!("task-{index}"),
            url: format!("https://example.com/{index}.m3u8"),
            save_name: Some(format!("save-{index}")),
            headers: None,
            status,
            retry_count: 0,
            progress: 1.0,
            speed: String::new(),
            threads: String::new(),
            output_path: Some(format!("D:/Videos/{index}.mp4")),
            error_message: None,
            created_at: Utc::now(),
            log_lines: VecDeque::new(),
        }
    }

    #[test]
    fn get_page_returns_newest_tasks_first() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        for index in 0..25 {
            let task = build_task(index, TaskStatus::Completed);
            store.append(&task).expect("append history task");
        }

        let page = store
            .get_page(HistoryStatus::Completed, 0, 20)
            .expect("read history page");

        assert_eq!(page.tasks.len(), 20);
        assert_eq!(
            page.tasks.first().map(|task| task.id.as_str()),
            Some("task-24")
        );
        assert_eq!(
            page.tasks.last().map(|task| task.id.as_str()),
            Some("task-5")
        );
        assert!(page.has_more);
        assert_eq!(page.next_offset, 20);

        fs::remove_dir_all(path).expect("cleanup history dir");
    }

    #[test]
    fn find_task_reads_from_history_chunks() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        for index in 0..12 {
            let task = build_task(index, TaskStatus::Failed);
            store.append(&task).expect("append history task");
        }

        let found = store
            .find_task(HistoryStatus::Failed, "task-3")
            .expect("search history");

        assert_eq!(found.map(|task| task.id), Some("task-3".to_string()));

        fs::remove_dir_all(path).expect("cleanup history dir");
    }

    #[test]
    fn append_creates_new_chunk_after_recovering_chunk_only_write() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        for index in 0..9 {
            let task = build_task(index, TaskStatus::Completed);
            store.append(&task).expect("append history task");
        }

        let chunk_path = path.join("completed").join("000001.json");
        let mut chunk = load_chunk(&chunk_path).expect("load chunk");
        chunk.push(build_task(9, TaskStatus::Completed));
        save_chunk_atomic(&chunk_path, &chunk).expect("simulate chunk write without index update");

        store
            .append(&build_task(10, TaskStatus::Completed))
            .expect("append after interrupted write");

        assert!(
            path.join("completed").join("000002.json").exists(),
            "append should reconcile the stale index before deciding the chunk target"
        );

        fs::remove_dir_all(path).expect("cleanup history dir");
    }
}
