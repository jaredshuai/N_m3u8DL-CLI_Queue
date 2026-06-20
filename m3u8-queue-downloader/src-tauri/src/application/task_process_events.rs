#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskLifecycleEvent {
    Completed { id: String, output_path: String },
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
