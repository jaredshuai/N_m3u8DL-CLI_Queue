use crate::app_error::{AppError, AppResult};
use crate::models::{HistoryPage, HistoryStatus, Task};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const HISTORY_CHUNK_SIZE: usize = 10;
const HISTORY_FORMAT_VERSION: &str = "2";

#[derive(Debug, Clone)]
pub struct HistoryStore {
    base_path: PathBuf,
    append_lock: Arc<Mutex<()>>,
    format_lock: Arc<Mutex<()>>,
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
            format_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("m3u8-queue-downloader")
            .join("history")
    }

    pub fn append(&self, task: &Task) -> AppResult<()> {
        self.ensure_format_current()?;
        let _guard = self
            .append_lock
            .lock()
            .map_err(|e| AppError::message(e.to_string()))?;
        let status = HistoryStatus::from_task_status(&task.status)?;
        let status_dir = self.status_dir(status);
        fs::create_dir_all(&status_dir)?;

        let mut index = self.load_index_for_append(status)?;

        let chunk_file = if let Some(last_chunk) = index.chunks.last() {
            if last_chunk.count < HISTORY_CHUNK_SIZE {
                last_chunk.file.clone()
            } else {
                let file = next_chunk_file_name(next_chunk_number(&index));
                index.chunks.push(HistoryChunkMeta {
                    file: file.clone(),
                    count: 0,
                });
                file
            }
        } else {
            let file = next_chunk_file_name(1);
            index.chunks.push(HistoryChunkMeta {
                file: file.clone(),
                count: 0,
            });
            file
        };

        let chunk_path = status_dir.join(&chunk_file);
        let mut chunk_tasks = load_chunk(&chunk_path)?;
        chunk_tasks.push(task.clone());
        save_chunk_atomic(&chunk_path, &chunk_tasks)?;

        if let Some(chunk) = index
            .chunks
            .iter_mut()
            .find(|chunk| chunk.file == chunk_file)
        {
            chunk.count = chunk_tasks.len();
        }
        index.total = index.chunks.iter().map(|chunk| chunk.count).sum();
        self.save_index_atomic(status, &index)
    }

    pub fn get_page(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> AppResult<HistoryPage> {
        self.ensure_format_current()?;
        if limit == 0 {
            return Ok(HistoryPage {
                tasks: Vec::new(),
                has_more: false,
                next_offset: offset,
            });
        }

        let index = self.load_index(status)?;
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

    pub fn find_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<Option<Task>> {
        self.ensure_format_current()?;
        let index = self.load_index(status)?;

        for chunk_meta in index.chunks.iter().rev() {
            let chunk_path = self.status_dir(status).join(&chunk_meta.file);
            let chunk_tasks = load_chunk(&chunk_path)?;
            if let Some(task) = chunk_tasks.into_iter().find(|task| task.id == task_id) {
                return Ok(Some(task));
            }
        }

        Ok(None)
    }

    pub fn remove_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<bool> {
        self.ensure_format_current()?;
        let _guard = self
            .append_lock
            .lock()
            .map_err(|e| AppError::message(e.to_string()))?;
        let mut index = self.load_index(status)?;
        let status_dir = self.status_dir(status);

        for chunk_index in 0..index.chunks.len() {
            let chunk = index.chunks[chunk_index].clone();
            let chunk_path = status_dir.join(&chunk.file);
            let mut tasks = load_chunk(&chunk_path)?;
            let original_len = tasks.len();
            tasks.retain(|task| task.id != task_id);

            if tasks.len() == original_len {
                continue;
            }

            if tasks.is_empty() {
                if chunk_path.exists() {
                    fs::remove_file(&chunk_path)?;
                }
                index.chunks.remove(chunk_index);
            } else {
                save_chunk_atomic(&chunk_path, &tasks)?;
                if let Some(meta) = index.chunks.get_mut(chunk_index) {
                    meta.count = tasks.len();
                }
            }

            index.total = index.chunks.iter().map(|chunk| chunk.count).sum();
            self.save_index_atomic(status, &index)?;
            return Ok(true);
        }

        Ok(false)
    }

    fn status_dir(&self, status: HistoryStatus) -> PathBuf {
        self.base_path.join(status.as_str())
    }

    fn format_version_path(&self) -> PathBuf {
        self.base_path.join("version.txt")
    }

    fn ensure_format_current(&self) -> AppResult<()> {
        if self.is_current_format()? {
            return Ok(());
        }

        let _guard = self
            .format_lock
            .lock()
            .map_err(|e| AppError::message(e.to_string()))?;
        if self.is_current_format()? {
            return Ok(());
        }

        reset_storage_root(&self.base_path)?;
        write_atomic(
            &self.format_version_path(),
            HISTORY_FORMAT_VERSION.as_bytes(),
        )
    }

    fn is_current_format(&self) -> AppResult<bool> {
        let version_path = self.format_version_path();
        if !version_path.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(version_path)?;
        Ok(content.trim() == HISTORY_FORMAT_VERSION)
    }

    fn index_path(&self, status: HistoryStatus) -> PathBuf {
        self.status_dir(status).join("index.json")
    }

    fn load_index(&self, status: HistoryStatus) -> AppResult<HistoryIndex> {
        let index_path = self.index_path(status);
        if !index_path.exists() {
            return Ok(HistoryIndex::default());
        }

        let content = fs::read_to_string(index_path)?;
        serde_json::from_str(&content).map_err(Into::into)
    }

    fn load_index_for_append(&self, status: HistoryStatus) -> AppResult<HistoryIndex> {
        let mut index = self.load_index(status)?;
        if let Some(last_chunk) = index.chunks.last_mut() {
            let chunk_path = self.status_dir(status).join(&last_chunk.file);
            last_chunk.count = load_chunk(&chunk_path)?.len();
            index.total = index.chunks.iter().map(|chunk| chunk.count).sum();
        }
        Ok(index)
    }

    fn save_index_atomic(&self, status: HistoryStatus, index: &HistoryIndex) -> AppResult<()> {
        let path = self.index_path(status);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(index)?;
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

fn reset_storage_root(path: &Path) -> AppResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path).map_err(Into::into)
}

fn load_chunk(path: &Path) -> AppResult<Vec<Task>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(Into::into)
}

fn save_chunk_atomic(path: &Path, tasks: &[Task]) -> AppResult<()> {
    let content = serde_json::to_string_pretty(tasks)?;
    write_atomic(path, content.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| AppError::message("missing file name for atomic write"))?
        .to_string_lossy();
    let tmp_path = path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()));

    fs::write(&tmp_path, bytes)?;
    replace_file_atomically(&tmp_path, path)
}

#[cfg(target_os = "windows")]
fn replace_file_atomically(tmp_path: &Path, path: &Path) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    if !path.exists() {
        return fs::rename(tmp_path, path).map_err(Into::into);
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
        let err = std::io::Error::last_os_error();
        let _ = fs::remove_file(tmp_path);
        return Err(err.into());
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_file_atomically(tmp_path: &Path, path: &Path) -> AppResult<()> {
    fs::rename(tmp_path, path).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TaskStatus;
    use chrono::Utc;
    use std::thread;
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

    #[test]
    fn get_page_trusts_index_without_scanning_unindexed_chunks() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        store
            .append(&build_task(0, TaskStatus::Completed))
            .expect("append history task");
        fs::write(path.join("completed").join("000999.json"), "{broken")
            .expect("write unindexed broken chunk");

        let page = store
            .get_page(HistoryStatus::Completed, 0, 20)
            .expect("read history page");

        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.tasks[0].id, "task-0");

        fs::remove_dir_all(path).expect("cleanup history dir");
    }

    #[test]
    fn append_trusts_index_without_scanning_unindexed_chunks() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        store
            .append(&build_task(0, TaskStatus::Completed))
            .expect("append history task");
        fs::write(path.join("completed").join("000999.json"), "{broken")
            .expect("write unindexed broken chunk");

        store
            .append(&build_task(1, TaskStatus::Completed))
            .expect("append should ignore unindexed chunks");

        let page = store
            .get_page(HistoryStatus::Completed, 0, 20)
            .expect("read history page");
        assert_eq!(page.tasks.len(), 2);

        fs::remove_dir_all(path).expect("cleanup history dir");
    }

    #[test]
    fn append_is_serialized_inside_process() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());
        let mut handles = Vec::new();

        for index in 0..40 {
            let store = store.clone();
            handles.push(thread::spawn(move || {
                let task = build_task(index, TaskStatus::Completed);
                store.append(&task).expect("append history task");
            }));
        }

        for handle in handles {
            handle.join().expect("join append thread");
        }

        let page = store
            .get_page(HistoryStatus::Completed, 0, 40)
            .expect("read history page");
        assert_eq!(page.tasks.len(), 40);

        fs::remove_dir_all(path).expect("cleanup history dir");
    }

    #[test]
    fn remove_task_deletes_record_and_updates_index() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        for index in 0..3 {
            let task = build_task(index, TaskStatus::Failed);
            store.append(&task).expect("append history task");
        }

        let removed = store
            .remove_task(HistoryStatus::Failed, "task-1")
            .expect("remove history task");
        assert!(removed);

        let found = store
            .find_task(HistoryStatus::Failed, "task-1")
            .expect("find removed task");
        assert!(found.is_none());

        let page = store
            .get_page(HistoryStatus::Failed, 0, 20)
            .expect("read history page");
        assert_eq!(page.tasks.len(), 2);
        assert_eq!(page.next_offset, 2);

        fs::remove_dir_all(path).expect("cleanup history dir");
    }

    #[test]
    fn get_page_discards_unversioned_history_directory() {
        let path = temp_history_path();
        let status_dir = path.join("completed");
        fs::create_dir_all(&status_dir).expect("create completed history dir");
        save_chunk_atomic(
            &status_dir.join("000001.json"),
            &[build_task(0, TaskStatus::Completed)],
        )
        .expect("write legacy history chunk");

        let store = HistoryStore::new(path.clone());
        let page = store
            .get_page(HistoryStatus::Completed, 0, 20)
            .expect("read page after purge");

        assert!(page.tasks.is_empty());
        assert!(path.join("version.txt").is_file());
        assert!(
            !status_dir.join("000001.json").exists(),
            "legacy chunk should be removed instead of loaded"
        );

        fs::remove_dir_all(path).expect("cleanup history dir");
    }
}
