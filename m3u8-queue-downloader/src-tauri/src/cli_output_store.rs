use crate::models::CliOutputPage;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const DEFAULT_PAGE_LIMIT: usize = 200;

#[derive(Debug, Clone)]
pub struct CliOutputStore {
    base_path: PathBuf,
    append_lock: Arc<Mutex<()>>,
    active_lines: Arc<Mutex<HashMap<String, String>>>,
}

impl CliOutputStore {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            append_lock: Arc::new(Mutex::new(())),
            active_lines: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("m3u8-queue-downloader")
            .join("cli-output")
    }

    pub fn append_line(&self, task_id: &str, line: &str) -> Result<(), String> {
        let _guard = self.append_lock.lock().map_err(|e| e.to_string())?;
        let path = self.task_path(task_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;

        use std::io::Write;
        writeln!(file, "{}", line).map_err(|e| e.to_string())
    }

    pub fn page(
        &self,
        task_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<CliOutputPage, String> {
        let limit = normalize_limit(limit);
        let lines = self.read_all_lines(task_id)?;
        let total = lines.len();
        let start = offset.min(total);
        let end = (start + limit).min(total);

        Ok(CliOutputPage {
            lines: lines[start..end].to_vec(),
            offset: start,
            total,
            next_offset: end,
            has_more_before: start > 0,
            has_more_after: end < total,
        })
    }

    pub fn tail(&self, task_id: &str, limit: usize) -> Result<CliOutputPage, String> {
        let limit = normalize_limit(limit);
        let lines = self.read_all_lines(task_id)?;
        let total = lines.len();
        let start = total.saturating_sub(limit);

        Ok(CliOutputPage {
            lines: lines[start..total].to_vec(),
            offset: start,
            total,
            next_offset: total,
            has_more_before: start > 0,
            has_more_after: false,
        })
    }

    pub fn set_active_line(&self, task_id: &str, line: String) {
        if let Ok(mut map) = self.active_lines.lock() {
            map.insert(task_id.to_string(), line);
        }
    }

    pub fn clear_active_line(&self, task_id: &str) {
        if let Ok(mut map) = self.active_lines.lock() {
            map.remove(task_id);
        }
    }

    pub fn get_active_line(&self, task_id: &str) -> Option<String> {
        self.active_lines.lock().ok()?.get(task_id).cloned()
    }

    fn task_path(&self, task_id: &str) -> PathBuf {
        let safe_id: String = task_id
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .collect();
        self.base_path.join(format!("{safe_id}.txt"))
    }

    fn read_all_lines(&self, task_id: &str) -> Result<Vec<String>, String> {
        let path = self.task_path(task_id);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        Ok(content.lines().map(str::to_string).collect())
    }
}

fn normalize_limit(limit: usize) -> usize {
    if limit == 0 {
        DEFAULT_PAGE_LIMIT
    } else {
        limit.min(2000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_output_path() -> PathBuf {
        std::env::temp_dir().join(format!("cli-output-{}", Uuid::new_v4()))
    }

    #[test]
    fn tail_returns_recent_lines_without_losing_total() {
        let path = temp_output_path();
        let store = CliOutputStore::new(path.clone());

        for i in 0..5 {
            store
                .append_line("task-1", &format!("line-{i}"))
                .expect("append line");
        }

        let page = store.tail("task-1", 2).expect("tail page");
        assert_eq!(page.total, 5);
        assert_eq!(page.offset, 3);
        assert_eq!(page.lines, vec!["line-3".to_string(), "line-4".to_string()]);
        assert!(page.has_more_before);
        assert!(!page.has_more_after);

        fs::remove_dir_all(path).expect("cleanup output dir");
    }

    #[test]
    fn active_line_is_stored_in_memory_only() {
        let path = temp_output_path();
        let store = CliOutputStore::new(path.clone());

        store.set_active_line("task-1", "Progress: 50%".to_string());
        assert_eq!(
            store.get_active_line("task-1").as_deref(),
            Some("Progress: 50%")
        );

        // Active line is NOT in the persisted file
        let page = store.tail("task-1", 100).expect("tail page");
        assert_eq!(page.total, 0);

        store.clear_active_line("task-1");
        assert_eq!(store.get_active_line("task-1"), None);

        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn page_reads_middle_slice() {
        let path = temp_output_path();
        let store = CliOutputStore::new(path.clone());

        for i in 0..5 {
            store
                .append_line("task-1", &format!("line-{i}"))
                .expect("append line");
        }

        let page = store.page("task-1", 1, 2).expect("page");
        assert_eq!(page.lines, vec!["line-1".to_string(), "line-2".to_string()]);
        assert!(page.has_more_before);
        assert!(page.has_more_after);

        fs::remove_dir_all(path).expect("cleanup output dir");
    }
}
