//! Artifact resolution: the three-valued result of consuming an
//! `ArtifactDirectorySnapshot` with `locate_artifact`.
//!
//! This is an application-internal type. It is **not** part of the
//! `TaskLifecycleEvent::Completed` payload — the event only carries the raw
//! facts (id / download_dir / save_name); `handle_completed_child_exit`
//! computes the resolution itself and projects it onto `TaskSnapshot`.

use crate::application::artifact_inventory::{ArtifactInventoryError, ArtifactPath};

/// Three-valued artifact location result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtifactResolution {
    /// An artifact was located at the given path.
    Located(ArtifactPath),
    /// No artifact matched the policy (directory was empty / missing, or no
    /// entry satisfied extension / prefix / freshness / kind constraints).
    NotFound,
    /// The inventory itself failed (permission denied, IO error, ...). The
    /// persisted `TaskSnapshot.output_path` will be `None`, but the
    /// diagnostic is retained so history can explain *why* it's `None`.
    InventoryUnavailable(ArtifactInventoryError),
}

/// Persisted-lightweight diagnostic explaining why `output_path: None` was
/// recorded for a completed task. Projected from `ArtifactResolution` and
/// stored on `TaskSnapshot` (mirrored to `StoredArtifactDiagnostic` in the
/// adapter layer for serialization).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactDiagnostic {
    pub(crate) kind: ArtifactDiagnosticKind,
    pub(crate) message: String,
}

/// Stable, serializable classification of artifact-inventory failures.
/// Distinct from `ArtifactInventoryErrorKind` (port) because this is the
/// application-owned view that persists in history; the port kind is an
/// implementation detail that may evolve with the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactDiagnosticKind {
    PermissionDenied,
    NotDirectory,
    Interrupted,
    Other,
}

impl From<&ArtifactInventoryError> for ArtifactDiagnostic {
    fn from(err: &ArtifactInventoryError) -> Self {
        Self {
            kind: match err.kind {
                crate::application::artifact_inventory::ArtifactInventoryErrorKind::PermissionDenied => {
                    ArtifactDiagnosticKind::PermissionDenied
                }
                crate::application::artifact_inventory::ArtifactInventoryErrorKind::NotDirectory => {
                    ArtifactDiagnosticKind::NotDirectory
                }
                crate::application::artifact_inventory::ArtifactInventoryErrorKind::Interrupted => {
                    ArtifactDiagnosticKind::Interrupted
                }
                crate::application::artifact_inventory::ArtifactInventoryErrorKind::Other => {
                    ArtifactDiagnosticKind::Other
                }
            },
            message: err.message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::artifact_inventory::{ArtifactDir, ArtifactInventoryErrorKind};

    fn sample_error(kind: ArtifactInventoryErrorKind) -> ArtifactInventoryError {
        ArtifactInventoryError::new(
            ArtifactDir::new("D:/Downloads".to_string()),
            kind,
            "boom".to_string(),
        )
    }

    #[test]
    fn diagnostic_kind_maps_each_port_kind() {
        assert!(matches!(
            ArtifactDiagnostic::from(&sample_error(ArtifactInventoryErrorKind::PermissionDenied)).kind,
            ArtifactDiagnosticKind::PermissionDenied
        ));
        assert!(matches!(
            ArtifactDiagnostic::from(&sample_error(ArtifactInventoryErrorKind::NotDirectory)).kind,
            ArtifactDiagnosticKind::NotDirectory
        ));
        assert!(matches!(
            ArtifactDiagnostic::from(&sample_error(ArtifactInventoryErrorKind::Interrupted)).kind,
            ArtifactDiagnosticKind::Interrupted
        ));
        assert!(matches!(
            ArtifactDiagnostic::from(&sample_error(ArtifactInventoryErrorKind::Other)).kind,
            ArtifactDiagnosticKind::Other
        ));
    }

    #[test]
    fn diagnostic_preserves_message() {
        let diag = ArtifactDiagnostic::from(&sample_error(ArtifactInventoryErrorKind::Other));
        assert_eq!(diag.message, "boom");
    }
}
