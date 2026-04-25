use crate::cli_output_store::CliOutputStore;
use crate::history_store::HistoryStore;
use crate::queue_manager::QueueManager;
use crate::settings_store::SettingsStore;
use crate::shutdown::ShutdownManager;
use crate::task_runner::TaskRunner;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub(crate) cli_output_store: Arc<CliOutputStore>,
    pub(crate) history_store: Arc<HistoryStore>,
    pub(crate) queue_manager: Arc<QueueManager>,
    pub(crate) settings_store: Arc<SettingsStore>,
    pub(crate) shutdown_manager: Arc<ShutdownManager>,
    pub(crate) task_runner: Arc<TaskRunner>,
}

impl AppState {
    pub fn new(
        cli_output_store: Arc<CliOutputStore>,
        history_store: Arc<HistoryStore>,
        queue_manager: Arc<QueueManager>,
        settings_store: Arc<SettingsStore>,
        shutdown_manager: Arc<ShutdownManager>,
        task_runner: Arc<TaskRunner>,
    ) -> Self {
        Self {
            cli_output_store,
            history_store,
            queue_manager,
            settings_store,
            shutdown_manager,
            task_runner,
        }
    }
}
