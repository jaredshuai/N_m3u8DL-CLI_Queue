use crate::app_error::{AppError, AppResult};
use crate::models::CliOutputPage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DEFAULT_PAGE_LIMIT: usize = 200;
const CLI_OUTPUT_CHUNK_SIZE: usize = 200;
const CLI_OUTPUT_FORMAT_VERSION: &str = "2";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CliOutputIndex {
    total: usize,
    chunks: Vec<CliChunkMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CliChunkMeta {
    file: String,
    count: usize,
}

#[derive(Debug, Clone)]
pub struct CliOutputStore {
    base_path: PathBuf,
    append_lock: Arc<Mutex<()>>,
    format_lock: Arc<Mutex<()>>,
    active_lines: Arc<Mutex<HashMap<String, String>>>,
}

impl CliOutputStore {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            append_lock: Arc::new(Mutex::new(())),
            format_lock: Arc::new(Mutex::new(())),
            active_lines: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("m3u8-queue-downloader")
            .join("cli-output")
    }

    pub fn append_line(&self, task_id: &str, line: &str) -> AppResult<()> {
        self.ensure_format_current()?;
        let _guard = self
            .append_lock
            .lock()
            .map_err(|e| AppError::message(e.to_string()))?;
        let task_dir = self.task_dir(task_id);
        fs::create_dir_all(&task_dir)?;

        let mut index = self.load_index_for_append(task_id)?;
        let chunk_file = match index.chunks.last_mut() {
            Some(last_chunk) if last_chunk.count < CLI_OUTPUT_CHUNK_SIZE => {
                last_chunk.count += 1;
                last_chunk.file.clone()
            }
            _ => {
                let file = next_chunk_file_name(index.chunks.len() + 1);
                index.chunks.push(CliChunkMeta {
                    file: file.clone(),
                    count: 1,
                });
                file
            }
        };

        append_chunk_line(&task_dir.join(&chunk_file), line)?;
        index.total += 1;
        self.save_index_atomic(task_id, &index)
    }

    pub fn page(&self, task_id: &str, offset: usize, limit: usize) -> AppResult<CliOutputPage> {
        self.ensure_format_current()?;
        let limit = normalize_limit(limit);
        let index = self.load_index(task_id)?;
        let total = index.total;
        let start = offset.min(total);
        let end = (start + limit).min(total);

        Ok(CliOutputPage {
            lines: self.read_range(task_id, &index, start, end)?,
            offset: start,
            total,
            next_offset: end,
            has_more_before: start > 0,
            has_more_after: end < total,
        })
    }

    pub fn tail(&self, task_id: &str, limit: usize) -> AppResult<CliOutputPage> {
        self.ensure_format_current()?;
        let limit = normalize_limit(limit);
        let index = self.load_index(task_id)?;
        let total = index.total;
        let start = total.saturating_sub(limit);

        Ok(CliOutputPage {
            lines: self.read_range(task_id, &index, start, total)?,
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

    fn task_dir(&self, task_id: &str) -> PathBuf {
        self.base_path.join(sanitize_task_id(task_id))
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
            CLI_OUTPUT_FORMAT_VERSION.as_bytes(),
        )
    }

    fn is_current_format(&self) -> AppResult<bool> {
        let version_path = self.format_version_path();
        if !version_path.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(version_path)?;
        Ok(content.trim() == CLI_OUTPUT_FORMAT_VERSION)
    }

    fn index_path(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("index.json")
    }

    fn load_index_for_append(&self, task_id: &str) -> AppResult<CliOutputIndex> {
        let task_dir = self.task_dir(task_id);
        let mut index = self.load_index(task_id)?;
        let mut changed = false;

        if index.chunks.is_empty() {
            let recovered = load_chunk_sequence_from_disk(&task_dir)?;
            if !recovered.chunks.is_empty() {
                index = recovered;
                changed = true;
            }
        } else if let Some(last_chunk) = index.chunks.last_mut() {
            let chunk_path = task_dir.join(&last_chunk.file);
            let actual_count = count_chunk_lines(&chunk_path)?;
            if actual_count != last_chunk.count {
                last_chunk.count = actual_count;
                changed = true;
            }

            let mut next_chunk_number = index.chunks.len() + 1;
            loop {
                let file = next_chunk_file_name(next_chunk_number);
                let path = task_dir.join(&file);
                if !path.exists() {
                    break;
                }

                let count = count_chunk_lines(&path)?;
                if count > 0 {
                    index.chunks.push(CliChunkMeta { file, count });
                    changed = true;
                }
                next_chunk_number += 1;
            }
        }

        index.total = index.chunks.iter().map(|chunk| chunk.count).sum();
        if changed {
            self.save_index_atomic(task_id, &index)?;
        }

        Ok(index)
    }

    fn load_index(&self, task_id: &str) -> AppResult<CliOutputIndex> {
        let path = self.index_path(task_id);
        if !path.exists() {
            return load_chunk_sequence_from_disk(&self.task_dir(task_id));
        }

        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(Into::into)
    }

    fn save_index_atomic(&self, task_id: &str, index: &CliOutputIndex) -> AppResult<()> {
        let path = self.index_path(task_id);
        let content = serde_json::to_string_pretty(index)?;
        write_atomic(&path, content.as_bytes())
    }

    fn read_range(
        &self,
        task_id: &str,
        index: &CliOutputIndex,
        start: usize,
        end: usize,
    ) -> AppResult<Vec<String>> {
        if start >= end {
            return Ok(Vec::new());
        }

        let task_dir = self.task_dir(task_id);
        let mut result = Vec::with_capacity(end - start);
        let mut chunk_start = 0usize;

        for chunk in &index.chunks {
            let chunk_end = chunk_start + chunk.count;
            if chunk_end <= start {
                chunk_start = chunk_end;
                continue;
            }
            if chunk_start >= end {
                break;
            }

            let local_start = start.saturating_sub(chunk_start);
            let local_end = end.min(chunk_end) - chunk_start;
            let lines = read_chunk_lines(&task_dir.join(&chunk.file))?;
            if lines.len() < local_end {
                return Err(AppError::message(format!(
                    "cli output chunk {} expected at least {} lines, found {}",
                    chunk.file,
                    local_end,
                    lines.len()
                )));
            }
            result.extend(lines[local_start..local_end].iter().cloned());
            chunk_start = chunk_end;
        }

        Ok(result)
    }
}

fn normalize_limit(limit: usize) -> usize {
    if limit == 0 {
        DEFAULT_PAGE_LIMIT
    } else {
        limit.min(2000)
    }
}

fn sanitize_task_id(task_id: &str) -> String {
    let safe_id: String = task_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();

    if safe_id.is_empty() {
        "task".to_string()
    } else {
        safe_id
    }
}

fn next_chunk_file_name(chunk_number: usize) -> String {
    format!("{chunk_number:06}.txt")
}

fn reset_storage_root(path: &Path) -> AppResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path).map_err(Into::into)
}

fn load_chunk_sequence_from_disk(task_dir: &Path) -> AppResult<CliOutputIndex> {
    if !task_dir.exists() {
        return Ok(CliOutputIndex::default());
    }

    let mut chunks = Vec::new();
    let mut next_chunk_number = 1usize;
    loop {
        let file = next_chunk_file_name(next_chunk_number);
        let path = task_dir.join(&file);
        if !path.exists() {
            break;
        }

        let count = count_chunk_lines(&path)?;
        if count > 0 {
            chunks.push(CliChunkMeta { file, count });
        }
        next_chunk_number += 1;
    }

    let total = chunks.iter().map(|chunk| chunk.count).sum();
    Ok(CliOutputIndex { total, chunks })
}

fn append_chunk_line(path: &Path, line: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(AppError::from)?;
    writeln!(file, "{}", line).map_err(AppError::from)
}

fn read_chunk_lines(path: &Path) -> AppResult<Vec<String>> {
    let file = fs::File::open(path)?;
    BufReader::new(file)
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn count_chunk_lines(path: &Path) -> AppResult<usize> {
    let file = fs::File::open(path)?;
    BufReader::new(file)
        .lines()
        .try_fold(0usize, |count, line| line.map(|_| count + 1))
        .map_err(Into::into)
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
    use serde_json::json;
    use uuid::Uuid;

    fn temp_output_path() -> PathBuf {
        std::env::temp_dir().join(format!("cli-output-{}", Uuid::new_v4()))
    }

    fn write_chunked_page_fixture(base_path: &PathBuf, task_id: &str) {
        let task_dir = base_path.join(task_id);
        fs::create_dir_all(&task_dir).expect("create task dir");
        fs::write(
            base_path.join("version.txt"),
            CLI_OUTPUT_FORMAT_VERSION.as_bytes(),
        )
        .expect("write version marker");
        fs::write(task_dir.join("000001.txt"), "line-0\nline-1\n").expect("write chunk 1");
        fs::write(task_dir.join("000002.txt"), "line-2\nline-3\n").expect("write chunk 2");
        fs::create_dir_all(task_dir.join("000003.txt")).expect("create broken chunk sentinel");
        fs::write(
            task_dir.join("index.json"),
            serde_json::to_vec_pretty(&json!({
                "total": 6,
                "chunks": [
                    { "file": "000001.txt", "count": 2 },
                    { "file": "000002.txt", "count": 2 },
                    { "file": "000003.txt", "count": 2 }
                ]
            }))
            .expect("serialize index"),
        )
        .expect("write index");
    }

    fn write_chunked_tail_fixture(base_path: &PathBuf, task_id: &str) {
        let task_dir = base_path.join(task_id);
        fs::create_dir_all(&task_dir).expect("create task dir");
        fs::write(
            base_path.join("version.txt"),
            CLI_OUTPUT_FORMAT_VERSION.as_bytes(),
        )
        .expect("write version marker");
        fs::create_dir_all(task_dir.join("000001.txt")).expect("create broken chunk sentinel");
        fs::write(task_dir.join("000002.txt"), "line-2\nline-3\n").expect("write chunk 2");
        fs::write(task_dir.join("000003.txt"), "line-4\nline-5\n").expect("write chunk 3");
        fs::write(
            task_dir.join("index.json"),
            serde_json::to_vec_pretty(&json!({
                "total": 6,
                "chunks": [
                    { "file": "000001.txt", "count": 2 },
                    { "file": "000002.txt", "count": 2 },
                    { "file": "000003.txt", "count": 2 }
                ]
            }))
            .expect("serialize index"),
        )
        .expect("write index");
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
    fn append_line_creates_chunked_task_directory() {
        let path = temp_output_path();
        let store = CliOutputStore::new(path.clone());

        store.append_line("task-1", "line-0").expect("append line");

        let task_dir = path.join("task-1");
        assert!(task_dir.is_dir(), "expected task output to be a directory");
        assert!(
            task_dir.join("index.json").is_file(),
            "expected task output index file to exist"
        );

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

    #[test]
    fn page_reads_only_requested_chunks() {
        let path = temp_output_path();
        let store = CliOutputStore::new(path.clone());
        write_chunked_page_fixture(&path, "task-1");

        let page = store.page("task-1", 0, 2).expect("page");
        assert_eq!(page.lines, vec!["line-0".to_string(), "line-1".to_string()]);
        assert_eq!(page.total, 6);
        assert_eq!(page.offset, 0);
        assert_eq!(page.next_offset, 2);
        assert!(!page.has_more_before);
        assert!(page.has_more_after);

        fs::remove_dir_all(path).expect("cleanup output dir");
    }

    #[test]
    fn tail_reads_only_recent_chunks() {
        let path = temp_output_path();
        let store = CliOutputStore::new(path.clone());
        write_chunked_tail_fixture(&path, "task-1");

        let page = store.tail("task-1", 2).expect("tail");
        assert_eq!(page.lines, vec!["line-4".to_string(), "line-5".to_string()]);
        assert_eq!(page.total, 6);
        assert_eq!(page.offset, 4);
        assert_eq!(page.next_offset, 6);
        assert!(page.has_more_before);
        assert!(!page.has_more_after);

        fs::remove_dir_all(path).expect("cleanup output dir");
    }

    #[test]
    fn tail_discards_unversioned_cli_output_directory() {
        let path = temp_output_path();
        fs::create_dir_all(&path).expect("create output dir");
        fs::write(path.join("task-1.txt"), "legacy-line\n").expect("write legacy cli file");

        let store = CliOutputStore::new(path.clone());
        let page = store.tail("task-1", 20).expect("tail after purge");

        assert_eq!(page.total, 0);
        assert!(path.join("version.txt").is_file());
        assert!(
            !path.join("task-1.txt").exists(),
            "legacy single-file output should be removed instead of loaded"
        );

        fs::remove_dir_all(path).expect("cleanup output dir");
    }
}
