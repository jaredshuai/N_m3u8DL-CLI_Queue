use crate::models::{HistoryPage, HistoryStatus, Task};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const HISTORY_CHUNK_SIZE: usize = 10;

#[derive(Debug, Clone)]
pub struct HistoryStore {
    base_path: PathBuf,
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
        Self { base_path }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("m3u8-queue-downloader")
            .join("history")
    }

    pub fn append(&self, task: &Task) -> Result<(), String> {
        let status = HistoryStatus::from_task_status(&task.status)?;
        let status_dir = self.status_dir(status);
        fs::create_dir_all(&status_dir).map_err(|e| e.to_string())?;

        let mut index = self.load_index(status)?;

        let chunk_file = if let Some(last_chunk) = index.chunks.last_mut() {
            if last_chunk.count < HISTORY_CHUNK_SIZE {
                last_chunk.count += 1;
                last_chunk.file.clone()
            } else {
                let file = next_chunk_file_name(index.chunks.len());
                index.chunks.push(HistoryChunkMeta {
                    file: file.clone(),
                    count: 1,
                });
                file
            }
        } else {
            let file = next_chunk_file_name(0);
            index.chunks.push(HistoryChunkMeta {
                file: file.clone(),
                count: 1,
            });
            file
        };

        let chunk_path = status_dir.join(&chunk_file);
        let mut chunk_tasks = load_chunk(&chunk_path)?;
        chunk_tasks.push(task.clone());
        save_chunk(&chunk_path, &chunk_tasks)?;

        index.total += 1;
        self.save_index(status, &index)
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

    pub fn find_task(&self, status: HistoryStatus, task_id: &str) -> Result<Option<Task>, String> {
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

    fn status_dir(&self, status: HistoryStatus) -> PathBuf {
        self.base_path.join(status.as_str())
    }

    fn index_path(&self, status: HistoryStatus) -> PathBuf {
        self.status_dir(status).join("index.json")
    }

    fn load_index(&self, status: HistoryStatus) -> Result<HistoryIndex, String> {
        let index_path = self.index_path(status);
        if !index_path.exists() {
            return Ok(HistoryIndex::default());
        }

        let content = fs::read_to_string(index_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    fn save_index(&self, status: HistoryStatus, index: &HistoryIndex) -> Result<(), String> {
        let path = self.index_path(status);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let content = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| e.to_string())
    }
}

fn next_chunk_file_name(existing_chunks: usize) -> String {
    format!("{:06}.json", existing_chunks + 1)
}

fn load_chunk(path: &Path) -> Result<Vec<Task>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn save_chunk(path: &Path, tasks: &[Task]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(tasks).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
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
        assert_eq!(page.tasks.first().map(|task| task.id.as_str()), Some("task-24"));
        assert_eq!(page.tasks.last().map(|task| task.id.as_str()), Some("task-5"));
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
}
