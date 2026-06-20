use crate::application::task_process_events::TaskOutputEvent;
use crate::application::Diagnostics;
use crate::application::FrontendEventPublisher;
use crate::application::QueueRepository;
use crate::application::TerminalOutputRepository;

pub(crate) struct TaskOutputEventPorts<'a> {
    queue_repository: &'a dyn QueueRepository,
    terminal_output_repository: &'a dyn TerminalOutputRepository,
    diagnostics: &'a dyn Diagnostics,
    events: &'a dyn FrontendEventPublisher,
}

impl<'a> TaskOutputEventPorts<'a> {
    pub(crate) fn new(
        queue_repository: &'a dyn QueueRepository,
        terminal_output_repository: &'a dyn TerminalOutputRepository,
        diagnostics: &'a dyn Diagnostics,
        events: &'a dyn FrontendEventPublisher,
    ) -> Self {
        Self {
            queue_repository,
            terminal_output_repository,
            diagnostics,
            events,
        }
    }

    pub(crate) async fn handle_task_output_event(&self, event: TaskOutputEvent) {
        self.dispatch_task_output_event(event).await;
    }

    async fn dispatch_task_output_event(&self, event: TaskOutputEvent) {
        match event {
            TaskOutputEvent::Progress {
                id,
                progress,
                speed,
                threads,
            } => {
                self.handle_task_progress(&id, progress, speed, threads)
                    .await;
            }
            TaskOutputEvent::TerminalCommittedLine { id, line } => {
                self.handle_terminal_committed_line(&id, &line);
            }
            TaskOutputEvent::TerminalActiveLine { id, active_line } => {
                self.handle_terminal_active_line(&id, &active_line);
            }
        }
    }

    async fn handle_task_progress(
        &self,
        id: &str,
        progress: Option<f32>,
        speed: Option<String>,
        threads: Option<String>,
    ) {
        let update_outcome = self
            .queue_repository
            .update_live_task_progress(id, progress, speed.clone(), threads.clone())
            .await;
        if update_outcome {
            self.mark_task_progress_recorded(id, progress, speed.as_deref(), threads.as_deref());
        }
    }

    fn mark_task_progress_recorded(
        &self,
        id: &str,
        progress: Option<f32>,
        speed: Option<&str>,
        threads: Option<&str>,
    ) {
        self.events.task_progress(id, progress, speed, threads);
    }

    fn handle_terminal_committed_line(&self, id: &str, line: &str) {
        if let Err(err) = self.terminal_output_repository.append_line(id, line) {
            self.mark_terminal_committed_line_persistence_failed(&err);
            return;
        }
        self.mark_terminal_committed_line_recorded(id, line);
    }

    fn mark_terminal_committed_line_recorded(&self, id: &str, line: &str) {
        self.events.terminal_committed_line(id, line);
    }

    fn mark_terminal_committed_line_persistence_failed(&self, error: &dyn std::fmt::Display) {
        self.diagnostics
            .warn(&format!("Failed to persist CLI live output: {}", error));
    }

    fn handle_terminal_active_line(&self, id: &str, active_line: &str) {
        self.sync_terminal_active_line(id, active_line);
        self.mark_terminal_active_line_recorded(id, active_line);
    }

    fn mark_terminal_active_line_recorded(&self, id: &str, active_line: &str) {
        self.events.terminal_active_line(id, active_line);
    }

    fn sync_terminal_active_line(&self, id: &str, active_line: &str) {
        if active_line.is_empty() {
            self.clear_terminal_active_line(id);
        } else {
            self.store_terminal_active_line(id, active_line);
        }
    }

    fn clear_terminal_active_line(&self, id: &str) {
        self.terminal_output_repository.clear_active_line(id);
    }

    fn store_terminal_active_line(&self, id: &str, active_line: &str) {
        self.terminal_output_repository
            .set_active_line(id, active_line.to_string());
    }
}
