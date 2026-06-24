use crate::adapters::history_status_codec::history_status_slug;
use crate::adapters::storage_files::{
    self, ChunkIndex as HistoryIndex, ChunkMeta as HistoryChunkMeta,
};
use crate::adapters::task_record::{
    stored_tasks_from_snapshots, stored_tasks_into_snapshots, StoredTask,
};
use crate::application::app_error::{AppError, AppResult};
use crate::application::history_repository_outcomes::{HistoryFindOutcome, HistoryRemoveOutcome};
use crate::application::history_task_page::HistoryTaskPage;
use crate::application::task_snapshot::{TaskSnapshot, TaskStatusSnapshot};
use crate::domain::history::HistoryStatus;
use crate::ports::history_repository::HistoryRepository;
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

    pub fn append(&self, task: &TaskSnapshot) -> AppResult<()> {
        self.ensure_format_current()?;
        let _guard = self
            .append_lock
            .lock()
            .map_err(|e| AppError::message(e.to_string()))?;
        let status = history_status_from_snapshot(task).ok_or(AppError::InvalidHistoryStatus)?;
        let status_dir = self.status_dir(status);
        fs::create_dir_all(&status_dir)?;

        let mut index = self.load_index_for_append(status)?;
        if self.index_contains_task(status, &index, &task.id)? {
            return Ok(());
        }

        let chunk_file = if let Some(last_chunk) = index.chunks.last() {
            if last_chunk.count < HISTORY_CHUNK_SIZE {
                last_chunk.file.clone()
            } else {
                let file =
                    storage_files::next_chunk_file_name(index.next_chunk_number("json"), "json");
                index.chunks.push(HistoryChunkMeta {
                    file: file.clone(),
                    count: 0,
                });
                file
            }
        } else {
            let file = storage_files::next_chunk_file_name(1, "json");
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
        index.recalculate_total();
        self.save_index_atomic(status, &index)
    }

    pub fn get_page(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> AppResult<HistoryTaskPage> {
        self.ensure_format_current()?;
        if limit == 0 {
            return Ok(HistoryTaskPage {
                tasks: Vec::new(),
                has_more: false,
                next_offset: offset,
            });
        }

        let index = self.load_index(status)?;
        if offset >= index.total {
            return Ok(HistoryTaskPage {
                tasks: Vec::new(),
                has_more: false,
                next_offset: offset,
            });
        }

        let mut remaining_skip = offset;
        let mut result = Vec::new();

        for chunk_meta in index.chunks.iter().rev() {
            if remaining_skip >= chunk_meta.count {
                remaining_skip -= chunk_meta.count;
                continue;
            }

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
                    return Ok(HistoryTaskPage {
                        has_more: next_offset < index.total,
                        next_offset,
                        tasks: result,
                    });
                }
            }
        }

        let next_offset = offset + result.len();
        Ok(HistoryTaskPage {
            has_more: next_offset < index.total,
            next_offset,
            tasks: result,
        })
    }

    pub fn find_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<HistoryFindOutcome> {
        self.ensure_format_current()?;
        let index = self.load_index(status)?;

        for chunk_meta in index.chunks.iter().rev() {
            let chunk_path = self.status_dir(status).join(&chunk_meta.file);
            let chunk_tasks = load_chunk(&chunk_path)?;
            if let Some(task) = chunk_tasks.into_iter().find(|task| task.id == task_id) {
                return Ok(HistoryFindOutcome::Found(task));
            }
        }

        Ok(HistoryFindOutcome::Missing)
    }

    pub fn remove_task(
        &self,
        status: HistoryStatus,
        task_id: &str,
    ) -> AppResult<HistoryRemoveOutcome> {
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

            index.recalculate_total();
            self.save_index_atomic(status, &index)?;
            return Ok(HistoryRemoveOutcome::Removed);
        }

        Ok(HistoryRemoveOutcome::Missing)
    }

    fn status_dir(&self, status: HistoryStatus) -> PathBuf {
        self.base_path.join(history_status_slug(status))
    }

    fn format_version_path(&self) -> PathBuf {
        self.base_path.join("version.txt")
    }

    fn ensure_format_current(&self) -> AppResult<()> {
        storage_files::ensure_format_current(
            &self.base_path,
            &self.format_version_path(),
            HISTORY_FORMAT_VERSION,
            self.format_lock.as_ref(),
        )
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
            index.recalculate_total();
        }
        Ok(index)
    }

    fn index_contains_task(
        &self,
        status: HistoryStatus,
        index: &HistoryIndex,
        task_id: &str,
    ) -> AppResult<bool> {
        for chunk_meta in index.chunks.iter().rev() {
            let chunk_path = self.status_dir(status).join(&chunk_meta.file);
            let chunk_tasks = load_chunk(&chunk_path)?;
            if chunk_tasks.iter().any(|task| task.id == task_id) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn save_index_atomic(&self, status: HistoryStatus, index: &HistoryIndex) -> AppResult<()> {
        let path = self.index_path(status);
        storage_files::save_json_atomic(&path, index)
    }
}

impl HistoryRepository for HistoryStore {
    fn append(&self, task: &TaskSnapshot) -> AppResult<()> {
        HistoryStore::append(self, task)
    }

    fn get_page(
        &self,
        status: HistoryStatus,
        offset: usize,
        limit: usize,
    ) -> AppResult<HistoryTaskPage> {
        HistoryStore::get_page(self, status, offset, limit)
    }

    fn find_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<HistoryFindOutcome> {
        HistoryStore::find_task(self, status, task_id)
    }

    fn remove_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<HistoryRemoveOutcome> {
        HistoryStore::remove_task(self, status, task_id)
    }
}

fn load_chunk(path: &Path) -> AppResult<Vec<TaskSnapshot>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    let tasks: Vec<StoredTask> = serde_json::from_str(&content)?;
    Ok(stored_tasks_into_snapshots(tasks))
}

fn save_chunk_atomic(path: &Path, tasks: &[TaskSnapshot]) -> AppResult<()> {
    let stored_tasks = stored_tasks_from_snapshots(tasks);
    storage_files::save_json_atomic(path, &stored_tasks)
}

fn history_status_from_snapshot(task: &TaskSnapshot) -> Option<HistoryStatus> {
    match &task.status {
        TaskStatusSnapshot::Completed => Some(HistoryStatus::Completed),
        TaskStatusSnapshot::Failed => Some(HistoryStatus::Failed),
        TaskStatusSnapshot::Waiting | TaskStatusSnapshot::Downloading | TaskStatusSnapshot::Cancelled => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::{Task, TaskStatus};
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
            error_message: None,
            created_at: Utc::now(),
        }
    }

    fn append_task(store: &HistoryStore, task: &Task) {
        store
            .append(&TaskSnapshot::from(task))
            .expect("append history task");
    }

    #[test]
    fn get_page_returns_newest_tasks_first() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        for index in 0..25 {
            let task = build_task(index, TaskStatus::Completed);
            append_task(&store, &task);
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
            append_task(&store, &task);
        }

        let found = store
            .find_task(HistoryStatus::Failed, "task-3")
            .expect("search history");

        assert!(matches!(
            found,
            HistoryFindOutcome::Found(task) if task.id == "task-3"
        ));

        fs::remove_dir_all(path).expect("cleanup history dir");
    }

    #[test]
    fn append_creates_new_chunk_after_recovering_chunk_only_write() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        for index in 0..9 {
            let task = build_task(index, TaskStatus::Completed);
            append_task(&store, &task);
        }

        let chunk_path = path.join("completed").join("000001.json");
        let mut chunk = load_chunk(&chunk_path).expect("load chunk");
        chunk.push(TaskSnapshot::from(&build_task(9, TaskStatus::Completed)));
        save_chunk_atomic(&chunk_path, &chunk).expect("simulate chunk write without index update");

        append_task(&store, &build_task(10, TaskStatus::Completed));

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

        append_task(&store, &build_task(0, TaskStatus::Completed));
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
    fn get_page_uses_index_counts_to_skip_unneeded_chunks() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        for index in 0..25 {
            append_task(&store, &build_task(index, TaskStatus::Completed));
        }
        fs::write(path.join("completed").join("000003.json"), "{broken")
            .expect("corrupt newest skipped chunk");

        let page = store
            .get_page(HistoryStatus::Completed, 5, 5)
            .expect("read page after skipping newest chunk");

        assert_eq!(
            page.tasks
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            vec!["task-19", "task-18", "task-17", "task-16", "task-15"]
        );

        fs::remove_dir_all(path).expect("cleanup history dir");
    }

    #[test]
    fn append_trusts_index_without_scanning_unindexed_chunks() {
        let path = temp_history_path();
        let store = HistoryStore::new(path.clone());

        append_task(&store, &build_task(0, TaskStatus::Completed));
        fs::write(path.join("completed").join("000999.json"), "{broken")
            .expect("write unindexed broken chunk");

        append_task(&store, &build_task(1, TaskStatus::Completed));

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
                append_task(&store, &task);
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
            append_task(&store, &task);
        }

        let removed = store
            .remove_task(HistoryStatus::Failed, "task-1")
            .expect("remove history task");
        assert!(matches!(removed, HistoryRemoveOutcome::Removed));

        let found = store
            .find_task(HistoryStatus::Failed, "task-1")
            .expect("find removed task");
        assert!(matches!(found, HistoryFindOutcome::Missing));

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
            &[TaskSnapshot::from(&build_task(0, TaskStatus::Completed))],
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
