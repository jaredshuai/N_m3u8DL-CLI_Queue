use crate::application::app_error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ChunkIndex {
    pub(crate) total: usize,
    pub(crate) chunks: Vec<ChunkMeta>,
}

impl ChunkIndex {
    pub(crate) fn recalculate_total(&mut self) {
        self.total = self.chunks.iter().map(|chunk| chunk.count).sum();
    }

    pub(crate) fn next_chunk_number(&self, extension: &str) -> usize {
        self.chunks
            .last()
            .and_then(|chunk| parse_chunk_number(&chunk.file, extension))
            .map(|value| value + 1)
            .unwrap_or(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChunkMeta {
    pub(crate) file: String,
    pub(crate) count: usize,
}

pub(crate) fn ensure_format_current(
    base_path: &Path,
    version_path: &Path,
    expected_version: &str,
    format_lock: &Mutex<()>,
) -> AppResult<()> {
    if is_current_format(version_path, expected_version)? {
        return Ok(());
    }

    let _guard = format_lock
        .lock()
        .map_err(|e| AppError::message(e.to_string()))?;
    if is_current_format(version_path, expected_version)? {
        return Ok(());
    }

    reset_storage_root(base_path)?;
    write_atomic(version_path, expected_version.as_bytes())
}

pub(crate) fn next_chunk_file_name(chunk_number: usize, extension: &str) -> String {
    format!("{chunk_number:06}.{extension}")
}

fn parse_chunk_number(file_name: &str, extension: &str) -> Option<usize> {
    file_name
        .strip_suffix(&format!(".{extension}"))
        .and_then(|value| value.parse::<usize>().ok())
}

pub(crate) fn save_json_atomic<T>(path: &Path, value: &T) -> AppResult<()>
where
    T: Serialize + ?Sized,
{
    let content = serde_json::to_string_pretty(value)?;
    write_atomic(path, content.as_bytes())
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
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

fn is_current_format(version_path: &Path, expected_version: &str) -> AppResult<bool> {
    if !version_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(version_path)?;
    Ok(content.trim() == expected_version)
}

fn reset_storage_root(path: &Path) -> AppResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path).map_err(Into::into)
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

    #[test]
    fn chunk_index_recalculates_total_from_chunk_counts() {
        let mut index = ChunkIndex {
            total: 999,
            chunks: vec![
                ChunkMeta {
                    file: "000001.txt".to_string(),
                    count: 2,
                },
                ChunkMeta {
                    file: "000002.txt".to_string(),
                    count: 3,
                },
            ],
        };

        index.recalculate_total();

        assert_eq!(index.total, 5);
    }

    #[test]
    fn chunk_index_picks_next_number_from_last_chunk_file() {
        let index = ChunkIndex {
            total: 0,
            chunks: vec![ChunkMeta {
                file: "000009.json".to_string(),
                count: 10,
            }],
        };

        assert_eq!(index.next_chunk_number("json"), 10);
        assert_eq!(index.next_chunk_number("txt"), 1);
    }
}
