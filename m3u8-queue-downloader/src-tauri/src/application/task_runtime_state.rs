/// Runtime state for a task that is being processed.
///
/// This is an application-layer concern: it tracks the live progress
/// of a task as observed from the external process, not the domain
/// concept of the task itself.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TaskRuntimeState {
    pub(crate) progress: f32,
    pub(crate) speed: String,
    pub(crate) threads: String,
    pub(crate) output_path: Option<String>,
}

impl TaskRuntimeState {
    pub(crate) fn empty() -> Self {
        Self {
            progress: 0.0,
            speed: String::new(),
            threads: String::new(),
            output_path: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_speed(mut self, speed: String) -> Self {
        if !speed.is_empty() {
            self.speed = speed;
        }
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_threads(mut self, threads: String) -> Self {
        if !threads.is_empty() {
            self.threads = threads;
        }
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_output_path(mut self, output_path: String) -> Self {
        self.output_path = Some(output_path);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn completed(output_path: String) -> Self {
        Self {
            progress: 1.0,
            speed: String::new(),
            threads: String::new(),
            output_path: Some(output_path),
        }
    }

    pub(crate) fn update_progress(
        &mut self,
        progress: Option<f32>,
        speed: Option<String>,
        threads: Option<String>,
    ) {
        if let Some(progress) = progress {
            self.progress = progress.clamp(0.0, 1.0);
        }
        if let Some(speed) = speed.filter(|v| !v.is_empty()) {
            self.speed = speed;
        }
        if let Some(threads) = threads.filter(|v| !v.is_empty()) {
            self.threads = threads;
        }
    }

    pub(crate) fn mark_completed(&mut self, output_path: &str) {
        self.progress = 1.0;
        self.output_path = Some(output_path.to_string());
    }

    #[allow(dead_code)]
    pub(crate) fn reset_for_restart(&mut self) {
        self.progress = 0.0;
        self.speed.clear();
        self.threads.clear();
        self.output_path = None;
    }

    pub(crate) fn reset_runtime_fields(&mut self) {
        self.progress = 0.0;
        self.speed.clear();
        self.threads.clear();
    }
}
