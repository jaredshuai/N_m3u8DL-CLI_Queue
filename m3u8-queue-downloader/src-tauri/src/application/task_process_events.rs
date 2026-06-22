use crate::application::artifact_inventory::ArtifactDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskLifecycleEvent {
    /// Child process exited successfully. Carries only raw facts the adapter
    /// observed — artifact location is computed by the application's
    /// `handle_completed_child_exit` (via `ArtifactInventory` + `locate_artifact`),
    /// not by the adapter. See ADR-0005 decision 5.
    Completed {
        id: String,
        download_dir: ArtifactDir,
        save_name: Option<String>,
    },
    Failed { id: String, error_message: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TaskOutputEvent {
    Progress {
        id: String,
        progress: Option<f32>,
        speed: Option<String>,
        threads: Option<String>,
    },
    TerminalCommittedLine {
        id: String,
        line: String,
    },
    TerminalActiveLine {
        id: String,
        active_line: String,
    },
}
