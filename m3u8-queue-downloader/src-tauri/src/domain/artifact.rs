/// A domain-level representation of a download task's output artifact.
///
/// ArtifactPackage captures the identity and metadata of what a task produced,
/// without knowing filesystem paths, file sizes, or storage details. The
/// concrete artifact location and validation are handled by the ArtifactStore
/// port in the infrastructure layer.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactPackage {
    task_id: String,
    artifact_ref: ArtifactRef,
}

/// An opaque reference to an artifact's location.
///
/// The domain does not know whether this is a filesystem path, a URL,
/// or a storage key. Infrastructure adapters interpret this reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtifactRef {
    Present(String),
    Absent,
}

#[allow(dead_code)]
impl ArtifactPackage {
    pub(crate) fn present(task_id: String, ref_value: String) -> Self {
        Self {
            task_id,
            artifact_ref: ArtifactRef::Present(ref_value),
        }
    }

    pub(crate) fn absent(task_id: String) -> Self {
        Self {
            task_id,
            artifact_ref: ArtifactRef::Absent,
        }
    }

    pub(crate) fn task_id(&self) -> &str {
        &self.task_id
    }

    pub(crate) fn is_present(&self) -> bool {
        matches!(self.artifact_ref, ArtifactRef::Present(_))
    }

    pub(crate) fn artifact_ref(&self) -> &ArtifactRef {
        &self.artifact_ref
    }

    pub(crate) fn ref_value(&self) -> Option<&str> {
        match &self.artifact_ref {
            ArtifactRef::Present(value) => Some(value.as_str()),
            ArtifactRef::Absent => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_artifact_has_ref_value() {
        let artifact = ArtifactPackage::present("task-1".to_string(), "output.mp4".to_string());
        assert!(artifact.is_present());
        assert_eq!(artifact.ref_value(), Some("output.mp4"));
        assert_eq!(artifact.task_id(), "task-1");
    }

    #[test]
    fn absent_artifact_has_no_ref() {
        let artifact = ArtifactPackage::absent("task-1".to_string());
        assert!(!artifact.is_present());
        assert!(artifact.ref_value().is_none());
    }
}
