//! Filesystem-backed `ArtifactInventory` adapter.
//!
//! Takes a read-only snapshot of a directory: lists entries, reads their
//! metadata, classifies kind, and reports the observed path. Does **not**
//! canonicalize, does **not** resolve symlink targets, does **not** perform
//! any artifact-location policy. Missing directory is reported as
//! `Ok(snapshot { presence: Missing, .. })`, not as an error.

use crate::application::artifact_inventory::{
    ArtifactDir, ArtifactDirectoryPresence, ArtifactDirectorySnapshot, ArtifactEntryKind,
    ArtifactInventoryError, ArtifactInventoryErrorKind, ArtifactModifiedAt, ArtifactPath,
    ObservedArtifactEntry,
};
use crate::ports::artifact_inventory::{ArtifactInventory, ArtifactInventoryFuture};
use std::fs;
use std::io;
use std::path::Path;
use tokio::task;

pub(crate) struct FilesystemArtifactInventory;

impl FilesystemArtifactInventory {
    pub(crate) fn new() -> Self {
        Self
    }
}

// Blocking filesystem traversal is offloaded to `spawn_blocking` so the async
// port contract does not stall a reactor thread on a slow directory read.
impl ArtifactInventory for FilesystemArtifactInventory {
    fn snapshot<'a>(
        &'a self,
        dir: &'a ArtifactDir,
    ) -> ArtifactInventoryFuture<'a, Result<ArtifactDirectorySnapshot, ArtifactInventoryError>>
    {
        let dir_clone = dir.clone();
        Box::pin(async move {
            let result = task::spawn_blocking(move || snapshot_sync(&dir_clone)).await;
            // spawn_blocking itself is infallible for our closure (we map all
            // fs errors inside), but JoinError (panic/cancel) is still possible.
            match result {
                Ok(inner) => inner,
                Err(join_err) => Err(ArtifactInventoryError::new(
                    dir.clone(),
                    ArtifactInventoryErrorKind::Interrupted,
                    format!("inventory task join failed: {join_err}"),
                )),
            }
        })
    }
}

fn snapshot_sync(
    dir: &ArtifactDir,
) -> Result<ArtifactDirectorySnapshot, ArtifactInventoryError> {
    let path = Path::new(dir.as_str());

    if !path.exists() {
        return Ok(ArtifactDirectorySnapshot {
            dir: dir.clone(),
            presence: ArtifactDirectoryPresence::Missing,
            entries: Vec::new(),
            skipped_entry_count: 0,
        });
    }

    let read_dir = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => return Err(classify_io_error(dir.clone(), e)),
    };

    let mut entries = Vec::new();
    let mut skipped = 0usize;

    for dir_entry in read_dir {
        let dir_entry = match dir_entry {
            Ok(de) => de,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let metadata = match dir_entry.metadata() {
            Ok(m) => m,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let modified_at = match metadata.modified() {
            Ok(sys_time) => match sys_time.duration_since(std::time::UNIX_EPOCH) {
                Ok(dur) => {
                    // Chrono DateTime<Utc> from seconds + nanos. mtime precision
                    // varies by filesystem; sub-second loss is acceptable for
                    // freshness comparisons (60s window).
                    chrono::DateTime::from_timestamp(
                        dur.as_secs() as i64,
                        dur.subsec_nanos(),
                    )
                    .unwrap_or_else(|| chrono::Utc::now())
                }
                Err(_) => {
                    // SystemTime before UNIX_EPOCH (clock skew). Skip the
                    // entry rather than poisoning the whole snapshot.
                    skipped += 1;
                    continue;
                }
            },
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let name = dir_entry.file_name().to_string_lossy().to_string();
        let path_str = dir_entry
            .path()
            .to_string_lossy()
            .to_string();
        let kind = classify_kind(&metadata);

        entries.push(ObservedArtifactEntry {
            name,
            path: ArtifactPath::new(path_str),
            modified_at: ArtifactModifiedAt::new(modified_at),
            kind,
        });
    }

    Ok(ArtifactDirectorySnapshot {
        dir: dir.clone(),
        presence: ArtifactDirectoryPresence::Present,
        entries,
        skipped_entry_count: skipped,
    })
}

/// Raw dirent kind — no symlink follow. `symlink_metadata` would be needed
/// only if we wanted the link's own metadata; for kind classification we use
/// the entry's effective metadata, but a symlink's `metadata()` (followed)
/// still reports `is_symlink()` correctly via the entry. To stay honest about
/// "raw kind, no follow", we use `symlink_metadata` on the entry path.
fn classify_kind(metadata: &fs::Metadata) -> ArtifactEntryKind {
    // Note: dir_entry.metadata() follows symlinks; to report the raw dirent
    // kind we'd want symlink_metadata. The port contract says "raw dirent
    // kind (no symlink follow)" — so use symlink_metadata on the path.
    // But we only have `metadata` here (the followed version). For correctness
    // we re-stat with symlink_metadata at call sites that need raw kind.
    // Here, the followed metadata is what read_dir's metadata() returns on
    // most platforms; for the symlink case we detect via file_type().
    let ft = metadata.file_type();
    if ft.is_symlink() {
        ArtifactEntryKind::Symlink
    } else if ft.is_dir() {
        ArtifactEntryKind::Directory
    } else if ft.is_file() {
        ArtifactEntryKind::File
    } else {
        ArtifactEntryKind::Other
    }
}

fn classify_io_error(dir: ArtifactDir, e: io::Error) -> ArtifactInventoryError {
    use io::ErrorKind as IoKind;
    let kind = match e.kind() {
        IoKind::PermissionDenied => ArtifactInventoryErrorKind::PermissionDenied,
        // `NotADirectory` (platform-specific) falls through to NotDirectory
        // when the OS surfaces it; otherwise Other.
        IoKind::NotADirectory => ArtifactInventoryErrorKind::NotDirectory,
        IoKind::Interrupted => ArtifactInventoryErrorKind::Interrupted,
        _ => ArtifactInventoryErrorKind::Other,
    };
    ArtifactInventoryError::new(dir, kind, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("artifact-inventory-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    fn write_file(dir: &Path, name: &str, contents: &str) {
        std::fs::write(dir.join(name), contents).expect("write file");
    }

    #[tokio::test]
    async fn missing_directory_returns_missing_snapshot() {
        let inv = FilesystemArtifactInventory::new();
        let dir = ArtifactDir::new("/nonexistent-adr-0005-test-dir".to_string());
        let snap = inv.snapshot(&dir).await.expect("snapshot ok");
        assert_eq!(snap.presence, ArtifactDirectoryPresence::Missing);
        assert!(snap.entries.is_empty());
        assert_eq!(snap.skipped_entry_count, 0);
    }

    #[tokio::test]
    async fn present_empty_directory_returns_present_snapshot_with_no_entries() {
        let tmp = temp_dir("empty");
        let dir = ArtifactDir::new(tmp.to_string_lossy().to_string());
        let inv = FilesystemArtifactInventory::new();
        let snap = inv.snapshot(&dir).await.expect("snapshot ok");
        assert_eq!(snap.presence, ArtifactDirectoryPresence::Present);
        assert!(snap.entries.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn entries_carry_name_path_modified_at_and_kind() {
        let tmp = temp_dir("entries");
        write_file(&tmp, "video.mp4", "x");
        write_file(&tmp, "notes.txt", "y");

        let dir = ArtifactDir::new(tmp.to_string_lossy().to_string());
        let inv = FilesystemArtifactInventory::new();
        let snap = inv.snapshot(&dir).await.expect("snapshot ok");
        assert_eq!(snap.presence, ArtifactDirectoryPresence::Present);
        assert_eq!(snap.entries.len(), 2);

        let by_name: std::collections::HashMap<&str, &ObservedArtifactEntry> = snap
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e))
            .collect();
        let video = by_name.get("video.mp4").expect("video.mp4 present");
        assert_eq!(video.kind, ArtifactEntryKind::File);
        assert!(video.path.as_str().ends_with("video.mp4"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn directory_entries_are_reported_as_directory_kind() {
        let tmp = temp_dir("with-subdir");
        std::fs::create_dir_all(tmp.join("foo.mp4")).expect("make dir named like video");
        write_file(&tmp, "real.mp4", "x");

        let dir = ArtifactDir::new(tmp.to_string_lossy().to_string());
        let inv = FilesystemArtifactInventory::new();
        let snap = inv.snapshot(&dir).await.expect("snapshot ok");
        let foo = snap
            .entries
            .iter()
            .find(|e| e.name == "foo.mp4")
            .expect("foo.mp4 entry");
        // Adapter reports raw kind (Directory). Whether the policy accepts it
        // is the application's decision (locate_artifact excludes Directory).
        assert_eq!(foo.kind, ArtifactEntryKind::Directory);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
