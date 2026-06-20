/// A domain-level transcript of terminal output for a single task execution.
///
/// TerminalTranscript captures the structured view of what a CLI process
/// produced during its lifetime: committed lines, an active (in-progress)
/// line, and the total line count. It does not know about chunk files,
/// filesystem paths, or storage format versions.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalTranscript {
    task_id: String,
    line_count: usize,
    active_line: Option<String>,
}

#[allow(dead_code)]
impl TerminalTranscript {
    pub(crate) fn new(task_id: String) -> Self {
        Self {
            task_id,
            line_count: 0,
            active_line: None,
        }
    }

    pub(crate) fn with_line_count(task_id: String, line_count: usize) -> Self {
        Self {
            task_id,
            line_count,
            active_line: None,
        }
    }

    pub(crate) fn task_id(&self) -> &str {
        &self.task_id
    }

    pub(crate) fn line_count(&self) -> usize {
        self.line_count
    }

    pub(crate) fn active_line(&self) -> Option<&str> {
        self.active_line.as_deref()
    }

    pub(crate) fn commit_line(&mut self, _line: &str) {
        self.line_count += 1;
        self.active_line = None;
    }

    pub(crate) fn set_active_line(&mut self, line: String) {
        self.active_line = Some(line);
    }

    pub(crate) fn clear_active_line(&mut self) {
        self.active_line = None;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.line_count == 0 && self.active_line.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_transcript_is_empty() {
        let transcript = TerminalTranscript::new("task-1".to_string());
        assert!(transcript.is_empty());
        assert_eq!(transcript.line_count(), 0);
        assert!(transcript.active_line().is_none());
    }

    #[test]
    fn commit_line_increments_count_and_clears_active() {
        let mut transcript = TerminalTranscript::new("task-1".to_string());
        transcript.set_active_line("partial".to_string());
        transcript.commit_line("full line");

        assert_eq!(transcript.line_count(), 1);
        assert!(transcript.active_line().is_none());
        assert!(!transcript.is_empty());
    }

    #[test]
    fn active_line_tracks_in_progress_output() {
        let mut transcript = TerminalTranscript::new("task-1".to_string());
        transcript.set_active_line("downloading...".to_string());

        assert_eq!(transcript.active_line(), Some("downloading..."));
        assert!(!transcript.is_empty());

        transcript.clear_active_line();
        assert!(transcript.active_line().is_none());
    }

    #[test]
    fn with_line_count_reconstructs_from_persisted_state() {
        let transcript = TerminalTranscript::with_line_count("task-1".to_string(), 150);
        assert_eq!(transcript.line_count(), 150);
        assert!(transcript.active_line().is_none());
    }
}
