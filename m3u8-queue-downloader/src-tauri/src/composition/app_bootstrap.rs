use crate::adapters::cli_output_store::CliOutputStore;
use crate::adapters::filesystem_artifact_inventory::FilesystemArtifactInventory;
use crate::adapters::history_store::HistoryStore;
use crate::adapters::persistence::Persistence;
use crate::adapters::queue_manager::QueueManager;
use crate::adapters::settings_store::SettingsStore;
use crate::adapters::shutdown::ShutdownManager;
use crate::adapters::stderr_diagnostics::StderrDiagnostics;
use crate::adapters::system_clock::SystemClock;
use crate::adapters::system_directory_opener::SystemDirectoryOpener;
use crate::adapters::system_download_directory_resolver::SystemDownloadDirectoryResolver;
use crate::adapters::task_runner::{TaskRunner, TauriTaskProcessRunnerFactory};
use crate::adapters::tauri_application_control::TauriApplicationControl;
use crate::adapters::uuid_task_id_generator::UuidTaskIdGenerator;
use crate::composition::dependency_graph::DependencyGraph;
use crate::composition::event_handlers;
use crate::composition::pending_history_worker;
use crate::composition::tray;
use std::sync::Arc;
use tauri::Manager;

pub(crate) fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();
    let (lifecycle_sender, lifecycle_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (output_sender, output_receiver) = tokio::sync::mpsc::unbounded_channel();
    let task_runner = Arc::new(TaskRunner::with_event_senders(
        lifecycle_sender,
        output_sender,
    ));
    let task_process_runner_factory = Arc::new(TauriTaskProcessRunnerFactory::new(
        task_runner.clone(),
        app_handle.clone(),
    ));
    let state = DependencyGraph::new(
        Arc::new(CliOutputStore::new(CliOutputStore::default_path())),
        Arc::new(HistoryStore::new(HistoryStore::default_path())),
        Arc::new(QueueManager::new(Persistence::default_path())),
        Arc::new(SettingsStore::new(SettingsStore::default_path())),
        Arc::new(SystemDownloadDirectoryResolver),
        Arc::new(SystemDirectoryOpener),
        Arc::new(TauriApplicationControl::new(app_handle.clone())),
        Arc::new(ShutdownManager::new()),
        task_runner.clone(),
        task_process_runner_factory,
        Arc::new(UuidTaskIdGenerator),
        Arc::new(SystemClock),
        Arc::new(FilesystemArtifactInventory::new()),
        Arc::new(StderrDiagnostics),
    );

    app.manage(state.clone());
    tray::setup_tray(app)?;
    pending_history_worker::spawn_pending_history_flush(state.clone(), app_handle.clone());
    event_handlers::spawn_task_lifecycle_worker(
        app_handle.clone(),
        state.clone(),
        lifecycle_receiver,
    );
    event_handlers::spawn_task_output_worker(app_handle, state, output_receiver);
    Ok(())
}
