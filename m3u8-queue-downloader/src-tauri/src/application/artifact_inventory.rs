//! Application-owned data types for the artifact-inventory port.
//!
//! Per ADR-0002, port traits describe capabilities the application requires;
//! the outcome/data types they exchange belong to the application layer.
//! Physical placement is here so the port file (`ports/artifact_inventory.rs`)
//! contains only the trait declaration and stays free of enum/struct
//! definitions (the guard test `ports_layer_does_not_own_application_outcome_models`
//! enforces this). This mirrors the precedent set by
//! `application::queue_repository_outcomes` + `ports::queue_repository`.
//!
//! See `docs/adr/0005-artifact-location-sunk-to-application.md`.

use chrono::{DateTime, Utc};

/// A directory to be inventoried. Absolute, non-canonical — the adapter does
/// not canonicalize it (canonicalize introduces new failure modes and the
/// downstream consumer does not need a canonical path). Callers must ensure
/// absoluteness before constructing this (typically at the
/// `DownloadDirectoryResolver` seam, via `current_dir().join(...)` rather than
/// `canonicalize`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactDir(String);

impl ArtifactDir {
    pub(crate) fn new(path: String) -> Self {
        Self(path)
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// An observed path to an artifact. Non-canonical, does not resolve symlink
/// targets. Intended for persistence and display only — it is the path the
/// adapter saw via `entry.path()`, not a normalized form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactPath(String);

impl ArtifactPath {
    pub(crate) fn new(path: String) -> Self {
        Self(path)
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<ArtifactPath> for String {
    fn from(path: ArtifactPath) -> Self {
        path.0
    }
}

/// Moment at which an inventory is performed. Wall-clock time, injected from
/// the `Clock` port (used for the freshness-window comparison in
/// `locate_artifact`). Distinct from `ArtifactModifiedAt` (file mtime) — both
/// wrap `DateTime<Utc>` but carry different semantics and must not share a
/// type name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InventoryMoment(DateTime<Utc>);

impl InventoryMoment {
    pub(crate) fn new(at: DateTime<Utc>) -> Self {
        Self(at)
    }
    pub(crate) fn as_chrono(&self) -> DateTime<Utc> {
        self.0
    }
}

/// mtime of an observed artifact entry. Filesystem metadata, used for
/// freshness-window and ordering decisions. Distinct from `InventoryMoment`
/// (wall-clock at inventory time).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ArtifactModifiedAt(DateTime<Utc>);

impl ArtifactModifiedAt {
    pub(crate) fn new(at: DateTime<Utc>) -> Self {
        Self(at)
    }
    pub(crate) fn as_chrono(&self) -> DateTime<Utc> {
        self.0
    }
}

/// Raw dirent kind, as observed from the filesystem (no symlink follow).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// A single observed entry in the inventoried directory. Carries only
/// filesystem facts the policy needs: name (for extension/prefix matching),
/// path (the value to persist), modified_at (for freshness + ordering), kind
/// (to exclude directories / decide symlink eligibility). `size` is
/// deliberately omitted — no current policy needs it; it can be added
/// incrementally if a future policy does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObservedArtifactEntry {
    pub(crate) name: String,
    pub(crate) path: ArtifactPath,
    pub(crate) modified_at: ArtifactModifiedAt,
    pub(crate) kind: ArtifactEntryKind,
}

/// Whether the directory itself was present when the snapshot was taken.
/// `Missing` is **not** an error — the subprocess may have placed its output
/// elsewhere; the policy returns `None` for a missing directory. But `Missing`
/// is distinguished from an empty-but-present directory so diagnostics can
/// tell "no output here" from "config pointed at the wrong place".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactDirectoryPresence {
    Present,
    Missing,
}

/// A read-only snapshot of an artifact directory. `ArtifactInventory::snapshot`
/// returns this; the application policy `locate_artifact` consumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactDirectorySnapshot {
    pub(crate) dir: ArtifactDir,
    pub(crate) presence: ArtifactDirectoryPresence,
    pub(crate) entries: Vec<ObservedArtifactEntry>,
    /// Number of entries whose metadata could not be read (e.g. concurrent
    /// deletion, permission glitch). These entries are skipped (not included
    /// in `entries`) rather than failing the whole snapshot — inventory is
    /// best-effort at the per-entry level.
    pub(crate) skipped_entry_count: usize,
}

/// Stable classification of why reading a directory failed. `NotFound` is
/// **not** in here — a missing directory is reported via
/// `ArtifactDirectoryPresence::Missing` in a successful snapshot, not via an
/// error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtifactInventoryErrorKind {
    PermissionDenied,
    NotDirectory,
    Interrupted,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactInventoryError {
    pub(crate) dir: ArtifactDir,
    pub(crate) kind: ArtifactInventoryErrorKind,
    pub(crate) message: String,
}

impl ArtifactInventoryError {
    pub(crate) fn new(dir: ArtifactDir, kind: ArtifactInventoryErrorKind, message: String) -> Self {
        Self { dir, kind, message }
    }
}
