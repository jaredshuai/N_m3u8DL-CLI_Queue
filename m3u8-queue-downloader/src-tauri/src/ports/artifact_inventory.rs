//! Port: take a read-only snapshot of an artifact directory.
//!
//! Per ADR-0002, this file contains only the trait declaration. The data
//! types exchanged (snapshot / entries / errors / path / dir / ...) live in
//! `application::artifact_inventory` — that is where the application owns its
//! outcome models; the port only describes the capability.
//!
//! Implementations must:
//! - return `Ok(snapshot { presence: Missing, .. })` when the directory does
//!   not exist (not an error),
//! - return `Err(ArtifactInventoryError)` for permission/IO failures,
//! - skip individual entries whose metadata cannot be read, incrementing
//!   `skipped_entry_count`,
//! - report `ArtifactPath` as the observed `entry.path()` (non-canonical,
//!   symlink target not resolved),
//! - report `ArtifactEntryKind` as the raw dirent kind (no symlink follow),
//! - not perform any artifact-location policy.
//!
//! See `docs/adr/0005-artifact-location-sunk-to-application.md`.

use crate::application::artifact_inventory::{
    ArtifactDirectorySnapshot, ArtifactDir, ArtifactInventoryError,
};
use std::future::Future;
use std::pin::Pin;

pub(crate) type ArtifactInventoryFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) trait ArtifactInventory: Send + Sync {
    fn snapshot<'a>(
        &'a self,
        dir: &'a ArtifactDir,
    ) -> ArtifactInventoryFuture<'a, Result<ArtifactDirectorySnapshot, ArtifactInventoryError>>;
}
