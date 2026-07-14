use crate::adapters::progress_parser::{parse_progress, parse_speed, parse_threads};
use crate::adapters::terminal_parser::{decode_cli_bytes_lossy, TerminalBuffer};
use crate::application::app_error::{AppError, AppResult};
use crate::application::artifact_inventory::ArtifactDir;
use crate::application::process_runner_outcomes::{
    ProcessRunnerShutdownStatus, TaskTerminationClaim, TaskTerminationClaimOutcome,
};
use crate::application::task_process_events::{TaskLifecycleEvent, TaskOutputEvent};
use crate::application::task_process_start_request::TaskProcessStartRequest;
use crate::ports::process_runner::{
    ProcessRunnerFuture, TaskProcessRunner, TaskProcessRunnerFactory, TaskProcessSupervisor,
};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashMap as StdHashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{path::BaseDirectory, Manager};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::sync::{oneshot, watch, Mutex, OwnedMutexGuard};
#[cfg(unix)]
use tokio::time::sleep;
#[cfg(any(unix, test))]
use tokio::time::Instant;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessTerminationState {
    Running,
    Claimed,
    Committed,
    Aborted,
}

type KillProcessFuture =
    Pin<Box<dyn Future<Output = AppResult<KillProcessResult>> + Send + 'static>>;
type KillProcessOperation = Box<dyn FnOnce(u32) -> KillProcessFuture + Send + 'static>;

struct ProcessTerminationRequest {
    kill: KillProcessOperation,
    response_tx: oneshot::Sender<AppResult<()>>,
}

#[derive(Clone)]
struct RunningProcess {
    generation: u64,
    termination_tx: watch::Sender<ProcessTerminationState>,
    termination_request_tx: mpsc::UnboundedSender<ProcessTerminationRequest>,
}

struct ProcessWaitRegistration {
    pid: u32,
    generation: u64,
    termination_rx: watch::Receiver<ProcessTerminationState>,
    termination_request_rx: mpsc::UnboundedReceiver<ProcessTerminationRequest>,
}

#[cfg(test)]
struct PendingTestChild {
    child: Child,
    registration: ProcessWaitRegistration,
}

pub struct TaskRunner {
    running_processes: Arc<Mutex<StdHashMap<String, RunningProcess>>>,
    next_process_generation: AtomicU64,
    shutting_down: Arc<Mutex<bool>>,
    lifecycle_sender: Option<mpsc::UnboundedSender<TaskLifecycleEvent>>,
    output_sender: Option<mpsc::UnboundedSender<TaskOutputEvent>>,
    #[cfg(test)]
    pending_test_children: Arc<Mutex<HashMap<String, PendingTestChild>>>,
}

pub(crate) struct TauriTaskProcessRunner {
    task_runner: Arc<TaskRunner>,
    app_handle: tauri::AppHandle,
}

pub(crate) struct TauriTaskProcessRunnerFactory {
    task_runner: Arc<TaskRunner>,
    app_handle: tauri::AppHandle,
}

impl TauriTaskProcessRunner {
    pub(crate) fn new(task_runner: Arc<TaskRunner>, app_handle: tauri::AppHandle) -> Self {
        Self {
            task_runner,
            app_handle,
        }
    }
}

impl TauriTaskProcessRunnerFactory {
    pub(crate) fn new(task_runner: Arc<TaskRunner>, app_handle: tauri::AppHandle) -> Self {
        Self {
            task_runner,
            app_handle,
        }
    }
}

impl TaskProcessRunnerFactory for TauriTaskProcessRunnerFactory {
    fn create_process_runner(&self) -> Arc<dyn TaskProcessRunner> {
        Arc::new(TauriTaskProcessRunner::new(
            Arc::clone(&self.task_runner),
            self.app_handle.clone(),
        ))
    }
}

impl TaskProcessRunner for TauriTaskProcessRunner {
    fn start_task<'a>(
        &'a self,
        request: TaskProcessStartRequest,
    ) -> ProcessRunnerFuture<'a, AppResult<()>> {
        Box::pin(async move {
            self.task_runner
                .start_task(request, self.app_handle.clone())
                .await
        })
    }

    fn shutdown_status<'a>(&'a self) -> ProcessRunnerFuture<'a, ProcessRunnerShutdownStatus> {
        Box::pin(async move {
            if self.task_runner.is_shutting_down().await {
                ProcessRunnerShutdownStatus::ShuttingDown
            } else {
                ProcessRunnerShutdownStatus::Running
            }
        })
    }
}

impl TaskProcessSupervisor for TaskRunner {
    fn begin_shutdown<'a>(&'a self) -> ProcessRunnerFuture<'a, ()> {
        Box::pin(async move { TaskRunner::begin_shutdown(self).await })
    }

    fn terminate_all_running_processes<'a>(&'a self) -> ProcessRunnerFuture<'a, AppResult<()>> {
        Box::pin(async move { TaskRunner::terminate_all_running_processes(self).await })
    }

    fn claim_task_termination<'a>(
        &'a self,
        task_id: &'a str,
    ) -> ProcessRunnerFuture<'a, AppResult<TaskTerminationClaimOutcome>> {
        Box::pin(async move { TaskRunner::claim_task_termination(self, task_id).await })
    }

    fn abort_task_termination<'a>(
        &'a self,
        claim: &'a TaskTerminationClaim,
    ) -> ProcessRunnerFuture<'a, ()> {
        Box::pin(async move { TaskRunner::abort_task_termination(self, claim).await })
    }

    fn terminate_claimed_task<'a>(
        &'a self,
        claim: &'a TaskTerminationClaim,
    ) -> ProcessRunnerFuture<'a, AppResult<()>> {
        Box::pin(async move { TaskRunner::terminate_claimed_task(self, claim).await })
    }
}

const MAX_CLI_SEARCH_DEPTH: usize = 8;
const KILL_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const PROCESS_EXIT_CONFIRM_TIMEOUT: Duration = Duration::from_secs(5);
const TERMINATION_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(unix)]
const GRACEFUL_TERMINATION_TIMEOUT: Duration = Duration::from_secs(3);
#[cfg(unix)]
const PROCESS_GROUP_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[cfg(any(unix, test))]
fn remaining_until_deadline(deadline: Instant, now: Instant) -> Option<Duration> {
    deadline.checked_duration_since(now)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KillProcessResult {
    Killed,
    AlreadyExited,
}

async fn run_kill_future_with_timeout<Fut>(
    kill_future: Fut,
    kill_timeout: Duration,
) -> AppResult<KillProcessResult>
where
    Fut: Future<Output = AppResult<KillProcessResult>>,
{
    timeout(kill_timeout, kill_future).await.map_err(|_| {
        AppError::message("Timed out waiting for the OS process termination command")
    })?
}

#[cfg(any(target_os = "windows", test))]
async fn run_command_output_with_timeout(
    command: &mut tokio::process::Command,
    command_timeout: Duration,
    description: &str,
) -> AppResult<std::process::Output> {
    command.kill_on_drop(true);
    timeout(command_timeout, command.output())
        .await
        .map_err(|_| AppError::message(format!("Timed out waiting for {description}")))?
        .map_err(|err| AppError::message(format!("Failed to launch {description}: {err}")))
}

#[derive(Debug, Clone, PartialEq)]
struct ProgressSnapshot {
    progress: Option<f32>,
    speed: Option<String>,
    threads: Option<String>,
}

pub(crate) struct TaskStartPermit {
    _guard: OwnedMutexGuard<bool>,
}

impl TaskRunner {
    pub fn new() -> Self {
        Self {
            running_processes: Arc::new(Mutex::new(StdHashMap::new())),
            next_process_generation: AtomicU64::new(1),
            shutting_down: Arc::new(Mutex::new(false)),
            lifecycle_sender: None,
            output_sender: None,
            #[cfg(test)]
            pending_test_children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    pub fn with_lifecycle_sender(sender: mpsc::UnboundedSender<TaskLifecycleEvent>) -> Self {
        Self {
            running_processes: Arc::new(Mutex::new(StdHashMap::new())),
            next_process_generation: AtomicU64::new(1),
            shutting_down: Arc::new(Mutex::new(false)),
            lifecycle_sender: Some(sender),
            output_sender: None,
            #[cfg(test)]
            pending_test_children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_event_senders(
        lifecycle_sender: mpsc::UnboundedSender<TaskLifecycleEvent>,
        output_sender: mpsc::UnboundedSender<TaskOutputEvent>,
    ) -> Self {
        Self {
            running_processes: Arc::new(Mutex::new(StdHashMap::new())),
            next_process_generation: AtomicU64::new(1),
            shutting_down: Arc::new(Mutex::new(false)),
            lifecycle_sender: Some(lifecycle_sender),
            output_sender: Some(output_sender),
            #[cfg(test)]
            pending_test_children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) async fn begin_shutdown(&self) {
        let mut shutting_down = self.shutting_down.lock().await;
        *shutting_down = true;
    }

    pub(crate) async fn is_shutting_down(&self) -> bool {
        *self.shutting_down.lock().await
    }

    pub(crate) async fn acquire_start_permit(&self) -> AppResult<TaskStartPermit> {
        let guard = Arc::clone(&self.shutting_down).lock_owned().await;
        if *guard {
            return Err(AppError::message("Task runner is shutting down"));
        }
        Ok(TaskStartPermit { _guard: guard })
    }

    pub async fn start_task(
        &self,
        request: TaskProcessStartRequest,
        app_handle: tauri::AppHandle,
    ) -> AppResult<()> {
        let start_permit = self.acquire_start_permit().await?;
        let cli_path = self.find_cli_exe(&app_handle)?;
        let TaskProcessStartRequest {
            task_id,
            url,
            save_name,
            headers,
            download_dir,
        } = request;
        let download_dir = PathBuf::from(download_dir.into_string());

        let mut args: Vec<String> = Vec::new();
        args.push(url);
        args.push("--workDir".to_string());
        args.push(download_dir.to_string_lossy().to_string());

        if let Some(ref save_name) = save_name {
            if !save_name.is_empty() {
                args.push("--saveName".to_string());
                args.push(save_name.clone());
            }
        }

        if let Some(ref headers) = headers {
            if !headers.is_empty() {
                args.push("--headers".to_string());
                args.push(headers.clone());
            }
        }

        args.push("--enableDelAfterDone".to_string());

        let mut cmd = tokio::process::Command::new(&cli_path);
        cmd.args(&args)
            .current_dir(cli_path.parent().unwrap_or_else(|| Path::new(".")))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::message(format!("Failed to spawn CLI process: {}", e)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::message("Failed to capture stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::message("Failed to capture stderr"))?;

        let pid = child
            .id()
            .ok_or_else(|| AppError::message("Failed to get CLI process ID"))?;
        let registration = self.register_running_task(task_id.clone(), pid).await;

        let task_id_stdout = task_id.clone();
        let output_sender_stdout = self.output_sender.clone();
        tokio::spawn(async move {
            read_cli_stream(stdout, task_id_stdout, output_sender_stdout, None).await;
        });

        let task_id_stderr = task_id.clone();
        let output_sender_stderr = self.output_sender.clone();
        tokio::spawn(async move {
            read_cli_stream(
                stderr,
                task_id_stderr,
                output_sender_stderr,
                Some("[stderr] "),
            )
            .await;
        });

        self.spawn_wait_task(task_id, child, save_name, download_dir, registration);
        drop(start_permit);
        Ok(())
    }

    #[cfg(test)]
    pub async fn is_task_running(&self, task_id: &str) -> bool {
        let processes = self.running_processes.lock().await;
        processes.contains_key(task_id)
    }

    pub async fn terminate_all_running_processes(&self) -> AppResult<()> {
        let running = {
            let processes = self.running_processes.lock().await;
            processes
                .iter()
                .map(|(task_id, process)| (task_id.clone(), process.generation))
                .collect::<Vec<_>>()
        };

        let mut errors = Vec::new();
        for (task_id, generation) in &running {
            if let Err(err) = self
                .request_process_termination_with(task_id, *generation, false, kill_process)
                .await
            {
                errors.push(format!("Failed to terminate task {task_id}: {err}"));
            }
        }

        if !errors.is_empty() {
            return Err(AppError::message(errors.join("; ")));
        }

        Ok(())
    }

    pub async fn claim_task_termination(
        &self,
        task_id: &str,
    ) -> AppResult<TaskTerminationClaimOutcome> {
        let processes = self.running_processes.lock().await;
        let Some(process) = processes.get(task_id) else {
            return Ok(TaskTerminationClaimOutcome::AlreadyExited);
        };

        let termination_state = *process.termination_tx.borrow();
        match termination_state {
            ProcessTerminationState::Claimed => {
                return Ok(TaskTerminationClaimOutcome::AlreadyClaimed);
            }
            ProcessTerminationState::Committed => {}
            ProcessTerminationState::Running | ProcessTerminationState::Aborted => {
                process
                    .termination_tx
                    .send(ProcessTerminationState::Claimed)
                    .map_err(|_| AppError::message("Task process waiter is no longer available"))?;
            }
        }

        Ok(TaskTerminationClaimOutcome::Claimed(TaskTerminationClaim {
            task_id: task_id.to_string(),
            generation: process.generation,
        }))
    }

    pub async fn abort_task_termination(&self, claim: &TaskTerminationClaim) {
        let processes = self.running_processes.lock().await;
        if let Some(process) = processes
            .get(&claim.task_id)
            .filter(|process| process.generation == claim.generation)
        {
            let termination_state = *process.termination_tx.borrow();
            if termination_state == ProcessTerminationState::Claimed {
                let _ = process
                    .termination_tx
                    .send(ProcessTerminationState::Aborted);
            }
        }
    }

    pub async fn terminate_claimed_task(&self, claim: &TaskTerminationClaim) -> AppResult<()> {
        self.terminate_claimed_task_with(claim, kill_process).await
    }

    async fn terminate_claimed_task_with<F, Fut>(
        &self,
        claim: &TaskTerminationClaim,
        kill: F,
    ) -> AppResult<()>
    where
        F: FnOnce(u32) -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<KillProcessResult>> + Send + 'static,
    {
        self.request_process_termination_with(&claim.task_id, claim.generation, true, kill)
            .await
    }

    async fn request_process_termination_with<F, Fut>(
        &self,
        task_id: &str,
        generation: u64,
        require_claim: bool,
        kill: F,
    ) -> AppResult<()>
    where
        F: FnOnce(u32) -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<KillProcessResult>> + Send + 'static,
    {
        let response_rx = {
            let processes = self.running_processes.lock().await;
            let Some(process) = processes
                .get(task_id)
                .filter(|process| process.generation == generation)
            else {
                return Ok(());
            };
            let termination_state = *process.termination_tx.borrow();
            if require_claim
                && !matches!(
                    termination_state,
                    ProcessTerminationState::Claimed | ProcessTerminationState::Committed
                )
            {
                return Err(AppError::message(
                    "Task process termination claim is no longer active",
                ));
            }

            let (response_tx, response_rx) = oneshot::channel();
            let kill: KillProcessOperation =
                Box::new(move |pid| Box::pin(kill(pid)) as KillProcessFuture);
            process
                .termination_request_tx
                .send(ProcessTerminationRequest { kill, response_tx })
                .map_err(|_| AppError::message("Task process waiter is no longer available"))?;

            if termination_state != ProcessTerminationState::Committed {
                process
                    .termination_tx
                    .send(ProcessTerminationState::Committed)
                    .map_err(|_| AppError::message("Task process waiter is no longer available"))?;
            }

            response_rx
        };

        timeout(TERMINATION_REQUEST_TIMEOUT, response_rx)
            .await
            .map_err(|_| {
                AppError::message(format!("Timed out waiting for task {task_id} termination"))
            })?
            .map_err(|_| {
                AppError::message("Task process waiter closed before termination completed")
            })?
    }

    async fn register_running_task(&self, task_id: String, pid: u32) -> ProcessWaitRegistration {
        let generation = self.next_process_generation.fetch_add(1, Ordering::Relaxed);
        let (termination_tx, termination_rx) = watch::channel(ProcessTerminationState::Running);
        let (termination_request_tx, termination_request_rx) = mpsc::unbounded_channel();
        let mut processes = self.running_processes.lock().await;
        processes.insert(
            task_id,
            RunningProcess {
                generation,
                termination_tx,
                termination_request_tx,
            },
        );
        ProcessWaitRegistration {
            pid,
            generation,
            termination_rx,
            termination_request_rx,
        }
    }

    fn spawn_wait_task(
        &self,
        task_id: String,
        child: Child,
        save_name: Option<String>,
        download_dir: PathBuf,
        registration: ProcessWaitRegistration,
    ) {
        let running_processes = Arc::clone(&self.running_processes);
        let lifecycle_sender = self.lifecycle_sender.clone();

        tokio::spawn(async move {
            let mut child = child;
            let ProcessWaitRegistration {
                pid,
                generation,
                termination_rx,
                mut termination_request_rx,
            } = registration;
            let (result, termination_response) = wait_for_child_or_termination(
                &mut child,
                pid,
                &task_id,
                &mut termination_request_rx,
            )
            .await;
            let termination_state =
                resolve_process_exit(&running_processes, &task_id, generation, termination_rx)
                    .await;
            let exit_error = result.as_ref().err().cloned();

            // ADR-0005: the adapter no longer locates the artifact. It only
            // reports the raw facts (download_dir + save_name) and lets the
            // application's handle_completed_child_exit do the snapshot +
            // locate_artifact work.
            let event = if termination_state == ProcessTerminationState::Committed {
                TaskLifecycleEvent::Cancelled {
                    id: task_id,
                    error_message: "Stopped by user".to_string(),
                }
            } else {
                match result {
                    Ok(exit_status) if exit_status.success() => {
                        let download_dir_string = download_dir.to_string_lossy().to_string();
                        TaskLifecycleEvent::Completed {
                            id: task_id,
                            download_dir: ArtifactDir::new(download_dir_string),
                            save_name,
                        }
                    }
                    Ok(exit_status) => {
                        let error_msg = format!(
                            "Process exited with code: {}",
                            exit_status.code().unwrap_or(-1)
                        );
                        TaskLifecycleEvent::Failed {
                            id: task_id,
                            error_message: error_msg,
                        }
                    }
                    Err(error) => {
                        let error_msg = format!("Process error: {}", error);
                        TaskLifecycleEvent::Failed {
                            id: task_id,
                            error_message: error_msg,
                        }
                    }
                }
            };

            if let Some(sender) = lifecycle_sender {
                let _ = sender.send(event);
            }
            if let Some(response_tx) = termination_response {
                let _ = response_tx.send(Ok(()));
            }
            complete_pending_termination_requests(&mut termination_request_rx, exit_error);
        });
    }

    #[cfg(test)]
    pub(crate) async fn insert_running_task_for_test(&self, task_id: String, child: Child) {
        let pid = child.id().expect("test child pid");
        let registration = self.register_running_task(task_id.clone(), pid).await;

        let mut pending = self.pending_test_children.lock().await;
        pending.insert(
            task_id,
            PendingTestChild {
                child,
                registration,
            },
        );
    }

    #[cfg(test)]
    pub(crate) async fn begin_wait_for_test(&self, task_id: &str) {
        let PendingTestChild {
            child,
            registration,
        } = {
            let mut pending = self.pending_test_children.lock().await;
            pending.remove(task_id).expect("pending test child")
        };
        let running_processes = Arc::clone(&self.running_processes);
        let lifecycle_sender = self.lifecycle_sender.clone();
        let task_id = task_id.to_string();

        tokio::spawn(async move {
            let mut child = child;
            let ProcessWaitRegistration {
                pid,
                generation,
                termination_rx,
                mut termination_request_rx,
            } = registration;
            let (result, termination_response) = wait_for_child_or_termination(
                &mut child,
                pid,
                &task_id,
                &mut termination_request_rx,
            )
            .await;
            let termination_state =
                resolve_process_exit(&running_processes, &task_id, generation, termination_rx)
                    .await;
            let exit_error = result.as_ref().err().cloned();

            if let Some(sender) = lifecycle_sender {
                let event = if termination_state == ProcessTerminationState::Committed {
                    TaskLifecycleEvent::Cancelled {
                        id: task_id,
                        error_message: "Stopped by user".to_string(),
                    }
                } else {
                    match result {
                        Ok(status) if status.success() => TaskLifecycleEvent::Completed {
                            id: task_id,
                            // Test helper: no real download_dir is available here.
                            // The application-side test stub (NoopArtifactInventory)
                            // returns a Missing snapshot, so the artifact resolves
                            // to None — equivalent to the old empty-string behavior.
                            download_dir: ArtifactDir::new(String::new()),
                            save_name: None,
                        },
                        Ok(status) => TaskLifecycleEvent::Failed {
                            id: task_id,
                            error_message: format!(
                                "Process exited with code: {}",
                                status.code().unwrap_or(-1)
                            ),
                        },
                        Err(error) => TaskLifecycleEvent::Failed {
                            id: task_id,
                            error_message: format!("Process error: {error}"),
                        },
                    }
                };
                let _ = sender.send(event);
            }
            if let Some(response_tx) = termination_response {
                let _ = response_tx.send(Ok(()));
            }
            complete_pending_termination_requests(&mut termination_request_rx, exit_error);
        });
    }

    fn find_cli_exe(&self, app_handle: &tauri::AppHandle) -> AppResult<PathBuf> {
        let cli_name = "N_m3u8DL-CLI_v3.0.2.exe";
        let bundled_resource_name = format!("resources/{cli_name}");

        if let Ok(candidate) = app_handle
            .path()
            .resolve(&bundled_resource_name, BaseDirectory::Resource)
        {
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                if let Some(path) = find_cli_in_ancestors(exe_dir, cli_name, MAX_CLI_SEARCH_DEPTH) {
                    return Ok(path);
                }
            }
        }

        if let Ok(cwd) = std::env::current_dir() {
            let candidate = cwd.join(cli_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        Err(AppError::CliExecutableNotFound {
            name: cli_name.to_string(),
        })
    }
}

fn find_cli_in_ancestors(start_dir: &Path, cli_name: &str, max_depth: usize) -> Option<PathBuf> {
    let mut dir = Some(start_dir.to_path_buf());

    for _ in 0..=max_depth {
        let current = dir?;
        let candidate = current.join(cli_name);
        if candidate.exists() {
            return Some(candidate);
        }
        dir = current.parent().map(|parent| parent.to_path_buf());
    }

    None
}

impl Default for TaskRunner {
    fn default() -> Self {
        Self::new()
    }
}

async fn read_cli_stream<R>(
    mut stream: R,
    task_id: String,
    output_sender: Option<mpsc::UnboundedSender<TaskOutputEvent>>,
    log_prefix: Option<&'static str>,
) where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 4096];
    let mut term = TerminalBuffer::new();
    let mut last_progress: Option<ProgressSnapshot> = None;

    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                term.feed(&buffer[..n]);

                for line in term.take_committed() {
                    emit_committed_line(
                        &line,
                        &task_id,
                        &output_sender,
                        log_prefix,
                        &mut last_progress,
                    );
                }

                if should_emit_active_line(log_prefix) {
                    let active = term.active_line().trim().to_string();
                    emit_active_line(
                        &active,
                        &task_id,
                        &output_sender,
                        log_prefix,
                        &mut last_progress,
                    );
                }
            }
            Err(_) => break,
        }
    }

    term.finish();
    for line in term.take_committed() {
        emit_committed_line(
            &line,
            &task_id,
            &output_sender,
            log_prefix,
            &mut last_progress,
        );
    }
    if should_emit_active_line(log_prefix) {
        emit_active_line("", &task_id, &output_sender, log_prefix, &mut last_progress);
    }
}

fn emit_committed_line(
    segment: &str,
    task_id: &str,
    output_sender: &Option<mpsc::UnboundedSender<TaskOutputEvent>>,
    log_prefix: Option<&str>,
    last_progress: &mut Option<ProgressSnapshot>,
) {
    if segment.is_empty() {
        return;
    }

    if log_prefix.is_none() {
        emit_progress_if_changed(output_sender, task_id, segment, last_progress);
    }

    let line = if let Some(prefix) = log_prefix {
        format!("{prefix}{segment}")
    } else {
        segment.to_string()
    };

    send_output_event(
        output_sender,
        TaskOutputEvent::TerminalCommittedLine {
            id: task_id.to_string(),
            line,
        },
    );
}

fn should_emit_active_line(log_prefix: Option<&str>) -> bool {
    log_prefix.is_none()
}

fn emit_active_line(
    line: &str,
    task_id: &str,
    output_sender: &Option<mpsc::UnboundedSender<TaskOutputEvent>>,
    log_prefix: Option<&str>,
    last_progress: &mut Option<ProgressSnapshot>,
) {
    if !line.is_empty() && log_prefix.is_none() {
        emit_progress_if_changed(output_sender, task_id, line, last_progress);
    }

    let prefixed = if line.is_empty() {
        String::new()
    } else if let Some(prefix) = log_prefix {
        format!("{prefix}{line}")
    } else {
        line.to_string()
    };

    send_output_event(
        output_sender,
        TaskOutputEvent::TerminalActiveLine {
            id: task_id.to_string(),
            active_line: prefixed,
        },
    );
}

fn emit_progress_if_changed(
    output_sender: &Option<mpsc::UnboundedSender<TaskOutputEvent>>,
    task_id: &str,
    segment: &str,
    last_progress: &mut Option<ProgressSnapshot>,
) {
    let Some(snapshot) = parse_progress_snapshot(segment) else {
        return;
    };

    if last_progress.as_ref() == Some(&snapshot) {
        return;
    }

    *last_progress = Some(snapshot.clone());
    send_output_event(
        output_sender,
        TaskOutputEvent::Progress {
            id: task_id.to_string(),
            progress: snapshot.progress,
            speed: snapshot.speed,
            threads: snapshot.threads,
        },
    );
}

fn parse_progress_snapshot(segment: &str) -> Option<ProgressSnapshot> {
    let snapshot = ProgressSnapshot {
        progress: parse_progress(segment),
        speed: parse_speed(segment),
        threads: parse_threads(segment),
    };

    if snapshot.progress.is_some() || snapshot.speed.is_some() || snapshot.threads.is_some() {
        Some(snapshot)
    } else {
        None
    }
}

fn send_output_event(
    output_sender: &Option<mpsc::UnboundedSender<TaskOutputEvent>>,
    event: TaskOutputEvent,
) {
    if let Some(sender) = output_sender {
        let _ = sender.send(event);
    }
}

async fn wait_for_child_or_termination(
    child: &mut Child,
    pid: u32,
    task_id: &str,
    termination_request_rx: &mut mpsc::UnboundedReceiver<ProcessTerminationRequest>,
) -> (
    Result<ExitStatus, String>,
    Option<oneshot::Sender<AppResult<()>>>,
) {
    loop {
        tokio::select! {
            result = child.wait() => {
                return wait_for_reaped_child_process_group(
                    pid,
                    task_id,
                    result.map_err(|err| err.to_string()),
                    termination_request_rx,
                )
                .await;
            }
            request = termination_request_rx.recv() => {
                let Some(request) = request else {
                    return (
                        child.wait().await.map_err(|err| err.to_string()),
                        None,
                    );
                };
                match terminate_child_process(child, task_id, request.kill).await {
                    Ok(status) => return (Ok(status), Some(request.response_tx)),
                    Err(err) => {
                        let _ = request.response_tx.send(Err(err));
                    }
                }
            }
        }
    }
}

fn complete_pending_termination_requests(
    termination_request_rx: &mut mpsc::UnboundedReceiver<ProcessTerminationRequest>,
    exit_error: Option<String>,
) {
    while let Ok(request) = termination_request_rx.try_recv() {
        let result = match &exit_error {
            Some(error) => Err(AppError::message(format!(
                "Process waiter failed before termination completed: {error}"
            ))),
            None => Ok(()),
        };
        let _ = request.response_tx.send(result);
    }
}

#[cfg(any(unix, test))]
async fn complete_reaped_termination_request_with<F, Fut>(
    pid: u32,
    request: ProcessTerminationRequest,
    cleanup: F,
) -> Option<oneshot::Sender<AppResult<()>>>
where
    F: FnOnce(u32, KillProcessOperation) -> Fut,
    Fut: Future<Output = AppResult<()>>,
{
    let ProcessTerminationRequest { kill, response_tx } = request;
    match cleanup(pid, kill).await {
        Ok(()) => Some(response_tx),
        Err(err) => {
            let _ = response_tx.send(Err(err));
            None
        }
    }
}

async fn wait_for_child_exit(
    child: &mut Child,
    wait_timeout: Duration,
    task_id: &str,
) -> AppResult<ExitStatus> {
    match timeout(wait_timeout, child.wait()).await {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(err)) => Err(AppError::message(format!(
            "Failed waiting for task {task_id} process exit: {err}"
        ))),
        Err(_) => Err(AppError::message(format!(
            "Timed out waiting for task {task_id} process exit"
        ))),
    }
}

#[cfg(target_os = "windows")]
async fn terminate_child_process(
    child: &mut Child,
    task_id: &str,
    kill: KillProcessOperation,
) -> AppResult<ExitStatus> {
    let pid = child
        .id()
        .ok_or_else(|| AppError::message("Task process has already exited"))?;
    let _ = run_kill_future_with_timeout(kill(pid), KILL_COMMAND_TIMEOUT).await?;
    wait_for_child_exit(child, PROCESS_EXIT_CONFIRM_TIMEOUT, task_id).await
}

#[cfg(unix)]
async fn terminate_child_process(
    child: &mut Child,
    task_id: &str,
    kill: KillProcessOperation,
) -> AppResult<ExitStatus> {
    let pid = child
        .id()
        .ok_or_else(|| AppError::message("Task process has already exited"))?;
    let _ = run_kill_future_with_timeout(kill(pid), KILL_COMMAND_TIMEOUT).await?;

    let graceful_status = match timeout(GRACEFUL_TERMINATION_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => Some(status),
        Ok(Err(err)) => {
            return Err(AppError::message(format!(
                "Failed waiting for task {task_id} process exit: {err}"
            )));
        }
        Err(_) => None,
    };

    if let Some(status) = graceful_status {
        if wait_for_process_group_exit(pid, GRACEFUL_TERMINATION_TIMEOUT).await? {
            return Ok(status);
        }

        let _ = run_kill_future_with_timeout(force_kill_process(pid), KILL_COMMAND_TIMEOUT).await?;
        if wait_for_process_group_exit(pid, PROCESS_EXIT_CONFIRM_TIMEOUT).await? {
            return Ok(status);
        }
        return Err(AppError::message(format!(
            "Timed out waiting for task {task_id} process group to exit after SIGKILL"
        )));
    }

    let _ = run_kill_future_with_timeout(force_kill_process(pid), KILL_COMMAND_TIMEOUT).await?;
    let status = wait_for_child_exit(child, PROCESS_EXIT_CONFIRM_TIMEOUT, task_id).await?;
    if wait_for_process_group_exit(pid, PROCESS_EXIT_CONFIRM_TIMEOUT).await? {
        return Ok(status);
    }
    Err(AppError::message(format!(
        "Timed out waiting for task {task_id} process group to exit after SIGKILL"
    )))
}

#[cfg(unix)]
async fn terminate_reaped_process_group(
    pid: u32,
    task_id: &str,
    kill: KillProcessOperation,
) -> AppResult<()> {
    if !process_group_exists(pid).await? {
        return Ok(());
    }

    let _ = run_kill_future_with_timeout(kill(pid), KILL_COMMAND_TIMEOUT).await?;
    if wait_for_process_group_exit(pid, GRACEFUL_TERMINATION_TIMEOUT).await? {
        return Ok(());
    }

    let _ = run_kill_future_with_timeout(force_kill_process(pid), KILL_COMMAND_TIMEOUT).await?;
    if wait_for_process_group_exit(pid, PROCESS_EXIT_CONFIRM_TIMEOUT).await? {
        return Ok(());
    }
    Err(AppError::message(format!(
        "Timed out waiting for task {task_id} process group to exit after SIGKILL"
    )))
}

#[cfg(unix)]
async fn wait_for_reaped_child_process_group(
    pid: u32,
    task_id: &str,
    result: Result<ExitStatus, String>,
    termination_request_rx: &mut mpsc::UnboundedReceiver<ProcessTerminationRequest>,
) -> (
    Result<ExitStatus, String>,
    Option<oneshot::Sender<AppResult<()>>>,
) {
    loop {
        if let Ok(request) = termination_request_rx.try_recv() {
            if let Some(response_tx) =
                complete_reaped_termination_request_with(pid, request, |pid, kill| {
                    terminate_reaped_process_group(pid, task_id, kill)
                })
                .await
            {
                return (result, Some(response_tx));
            }
            continue;
        }

        match process_group_exists(pid).await {
            Ok(false) => return (result, None),
            Ok(true) => {}
            Err(err) => {
                return (
                    Err(format!(
                        "Failed to confirm task {task_id} process group exit: {err}"
                    )),
                    None,
                );
            }
        }

        tokio::select! {
            request = termination_request_rx.recv() => {
                let Some(request) = request else {
                    return (result, None);
                };
                if let Some(response_tx) = complete_reaped_termination_request_with(
                    pid,
                    request,
                    |pid, kill| terminate_reaped_process_group(pid, task_id, kill),
                )
                .await
                {
                    return (result, Some(response_tx));
                }
            }
            _ = sleep(PROCESS_GROUP_POLL_INTERVAL) => {}
        }
    }
}

#[cfg(not(unix))]
async fn wait_for_reaped_child_process_group(
    _pid: u32,
    _task_id: &str,
    result: Result<ExitStatus, String>,
    _termination_request_rx: &mut mpsc::UnboundedReceiver<ProcessTerminationRequest>,
) -> (
    Result<ExitStatus, String>,
    Option<oneshot::Sender<AppResult<()>>>,
) {
    (result, None)
}

#[cfg(unix)]
async fn wait_for_process_group_exit(pid: u32, wait_timeout: Duration) -> AppResult<bool> {
    let deadline = Instant::now() + wait_timeout;
    loop {
        if remaining_until_deadline(deadline, Instant::now()).is_none() {
            return Ok(false);
        }
        if !process_group_exists(pid).await? {
            return Ok(true);
        }
        let Some(remaining) = remaining_until_deadline(deadline, Instant::now()) else {
            return Ok(false);
        };
        sleep(remaining.min(PROCESS_GROUP_POLL_INTERVAL)).await;
    }
}

#[cfg(unix)]
async fn process_group_exists(pid: u32) -> AppResult<bool> {
    let process_group = process_group_target(pid)?;
    // SAFETY: libc::kill reads only the integer target and signal values.
    let result = unsafe { libc::kill(process_group, 0) };
    if result == 0 {
        return Ok(true);
    }

    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(AppError::message(format!(
            "Failed to probe process group {pid}: {error}"
        ))),
    }
}

#[cfg(unix)]
fn process_group_target(pid: u32) -> AppResult<libc::pid_t> {
    let pid = libc::pid_t::try_from(pid)
        .map_err(|_| AppError::message(format!("Process ID {pid} exceeds pid_t range")))?;
    if pid == 0 {
        return Err(AppError::message("Process ID must be non-zero"));
    }
    Ok(-pid)
}

async fn resolve_process_exit(
    running_processes: &Arc<Mutex<StdHashMap<String, RunningProcess>>>,
    task_id: &str,
    expected_generation: u64,
    mut termination_rx: watch::Receiver<ProcessTerminationState>,
) -> ProcessTerminationState {
    loop {
        {
            let mut processes = running_processes.lock().await;
            let state = processes
                .get(task_id)
                .filter(|process| process.generation == expected_generation)
                .map(|process| *process.termination_tx.borrow());

            match state {
                Some(ProcessTerminationState::Claimed) => {}
                Some(state) => {
                    processes.remove(task_id);
                    return state;
                }
                None => {
                    let state = *termination_rx.borrow();
                    return if state == ProcessTerminationState::Claimed {
                        ProcessTerminationState::Aborted
                    } else {
                        state
                    };
                }
            }
        }

        if termination_rx.changed().await.is_err() {
            return ProcessTerminationState::Aborted;
        }
    }
}

#[cfg(target_os = "windows")]
async fn kill_process(pid: u32) -> AppResult<KillProcessResult> {
    let mut cmd = tokio::process::Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let output =
        run_command_output_with_timeout(&mut cmd, KILL_COMMAND_TIMEOUT, "taskkill").await?;

    if output.status.success() {
        return Ok(KillProcessResult::Killed);
    }

    let stdout = decode_cli_bytes_lossy(&output.stdout).to_lowercase();
    let stderr = decode_cli_bytes_lossy(&output.stderr).to_lowercase();
    let combined = format!("{stdout}\n{stderr}");
    if combined.contains("not found")
        || combined.contains("no running instance")
        || combined.contains("没有找到")
        || combined.contains("找不到")
    {
        return Ok(KillProcessResult::AlreadyExited);
    }

    Err(AppError::message(format!(
        "taskkill exited with code {}: {}",
        output.status.code().unwrap_or(-1),
        combined.trim()
    )))
}

#[cfg(unix)]
async fn kill_process(pid: u32) -> AppResult<KillProcessResult> {
    send_process_group_signal(pid, libc::SIGTERM, "SIGTERM").await
}

#[cfg(unix)]
async fn force_kill_process(pid: u32) -> AppResult<KillProcessResult> {
    send_process_group_signal(pid, libc::SIGKILL, "SIGKILL").await
}

#[cfg(unix)]
async fn send_process_group_signal(
    pid: u32,
    signal: libc::c_int,
    signal_name: &str,
) -> AppResult<KillProcessResult> {
    let process_group = process_group_target(pid)?;
    // SAFETY: libc::kill reads only the integer target and signal values.
    let result = unsafe { libc::kill(process_group, signal) };
    if result == 0 {
        return Ok(KillProcessResult::Killed);
    }

    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(KillProcessResult::AlreadyExited),
        _ => Err(AppError::message(format!(
            "Failed to send {signal_name} to process group {pid}: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        find_cli_in_ancestors, read_cli_stream, should_emit_active_line, ProcessTerminationState,
        TaskRunner,
    };
    use crate::application::app_error::AppError;
    use crate::application::process_runner_outcomes::{
        TaskTerminationClaim, TaskTerminationClaimOutcome,
    };
    use crate::application::task_process_events::{TaskLifecycleEvent, TaskOutputEvent};
    use crate::test_support::{spawn_sleeping_child, spawn_success_child};
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::sync::mpsc;
    use tokio::time::timeout;
    use uuid::Uuid;

    #[cfg(target_os = "windows")]
    fn delayed_marker_command(marker: &std::path::Path) -> tokio::process::Command {
        let escaped_marker = marker.to_string_lossy().replace('\'', "''");
        let script = format!(
            "Start-Sleep -Milliseconds 750; Set-Content -LiteralPath '{escaped_marker}' -Value done"
        );
        let mut command = tokio::process::Command::new("powershell");
        command.args(["-NoProfile", "-Command", &script]);
        command
    }

    #[cfg(not(target_os = "windows"))]
    fn delayed_marker_command(marker: &std::path::Path) -> tokio::process::Command {
        let mut command = tokio::process::Command::new("sh");
        command.args([
            "-c",
            "sleep 0.75; printf done > \"$1\"",
            "delayed-marker",
            marker.to_string_lossy().as_ref(),
        ]);
        command
    }

    #[test]
    fn stderr_stream_does_not_drive_terminal_active_line() {
        assert!(should_emit_active_line(None));
        assert!(!should_emit_active_line(Some("[stderr] ")));
    }

    #[tokio::test]
    async fn cli_stream_sends_output_events_through_internal_channel() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (mut writer, reader) = tokio::io::duplex(1024);
        let task_id = "task-output".to_string();
        let line = "Progress: 1/4 (25.00%) -- 1.00MB/4.00MB (512 KB/s @ 00:00:06)";

        tokio::spawn(async move {
            writer
                .write_all(format!("{line}\n").as_bytes())
                .await
                .expect("write cli output");
        });

        read_cli_stream(reader, task_id.clone(), Some(tx), None).await;

        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }

        assert!(events.contains(&TaskOutputEvent::Progress {
            id: task_id.clone(),
            progress: Some(0.25),
            speed: Some("512 KB/s".to_string()),
            threads: None,
        }));
        assert!(events.contains(&TaskOutputEvent::TerminalCommittedLine {
            id: task_id,
            line: line.to_string(),
        }));
    }

    #[tokio::test]
    async fn cli_stream_deduplicates_active_and_committed_progress() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (mut writer, reader) = tokio::io::duplex(1024);
        let task_id = "task-progress-dedupe".to_string();
        let line = "Progress: 1/4 (25.00%) -- 1.00MB/4.00MB (512 KB/s @ 00:00:06)";

        tokio::spawn(async move {
            writer
                .write_all(format!("{line}\r").as_bytes())
                .await
                .expect("write cli output");
        });

        read_cli_stream(reader, task_id, Some(tx), None).await;

        let mut progress_events = 0;
        while let Some(event) = rx.recv().await {
            if matches!(event, TaskOutputEvent::Progress { .. }) {
                progress_events += 1;
            }
        }

        assert_eq!(progress_events, 1);
    }

    #[tokio::test]
    async fn running_task_remains_registered_until_process_exits() {
        let runner = TaskRunner::new();
        let task_id = "task-1".to_string();
        let child = spawn_sleeping_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        runner.begin_wait_for_test(&task_id).await;

        assert!(runner.is_task_running(&task_id).await);
    }

    #[tokio::test]
    async fn terminate_all_running_processes_clears_registered_tasks() {
        let runner = TaskRunner::new();
        let task_id = "task-terminate".to_string();
        let child = spawn_sleeping_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        runner.begin_wait_for_test(&task_id).await;

        runner
            .terminate_all_running_processes()
            .await
            .expect("terminate running processes");

        assert!(!runner.is_task_running(&task_id).await);
    }

    #[tokio::test]
    async fn shutdown_gate_rejects_new_start_permits() {
        let runner = TaskRunner::new();

        runner.begin_shutdown().await;

        assert!(runner.is_shutting_down().await);
        assert!(runner.acquire_start_permit().await.is_err());
    }

    #[tokio::test]
    async fn wait_task_sends_completion_through_internal_channel() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-complete".to_string();
        let child = spawn_success_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        runner.begin_wait_for_test(&task_id).await;

        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("lifecycle event timeout")
            .expect("lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Completed { id, .. } if id == task_id
        ));
    }

    #[tokio::test]
    async fn termination_claim_reports_already_exited_without_emitting_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);

        let outcome = runner
            .claim_task_termination("nonexistent")
            .await
            .expect("claim termination");

        assert!(matches!(
            outcome,
            TaskTerminationClaimOutcome::AlreadyExited
        ));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn claimed_termination_emits_cancelled_only_after_waiter_observes_exit() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-claimed-cancel".to_string();
        let child = spawn_sleeping_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        runner.begin_wait_for_test(&task_id).await;

        let TaskTerminationClaimOutcome::Claimed(claim) = runner
            .claim_task_termination(&task_id)
            .await
            .expect("claim termination")
        else {
            panic!("running child should be claimable");
        };

        assert!(timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err());

        runner
            .terminate_claimed_task(&claim)
            .await
            .expect("terminate claimed child");

        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("lifecycle event timeout")
            .expect("lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Cancelled { id, .. } if id == task_id
        ));
        assert!(!runner.is_task_running(&task_id).await);
    }

    #[tokio::test]
    async fn aborted_termination_claim_restores_natural_exit_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-aborted-cancel".to_string();
        let child = spawn_sleeping_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        runner.begin_wait_for_test(&task_id).await;

        let TaskTerminationClaimOutcome::Claimed(claim) = runner
            .claim_task_termination(&task_id)
            .await
            .expect("claim termination")
        else {
            panic!("running child should be claimable");
        };
        runner.abort_task_termination(&claim).await;

        let event = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("natural lifecycle event timeout")
            .expect("natural lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Completed { id, .. } if id == task_id
        ));
    }

    #[tokio::test]
    async fn kill_failure_is_returned_while_claimed_process_remains_tracked() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-kill-failure".to_string();
        let child = spawn_sleeping_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        runner.begin_wait_for_test(&task_id).await;
        let TaskTerminationClaimOutcome::Claimed(claim) = runner
            .claim_task_termination(&task_id)
            .await
            .expect("claim termination")
        else {
            panic!("running child should be claimable");
        };

        let result = runner
            .terminate_claimed_task_with(&claim, |_| async {
                Err(AppError::message("simulated permission denial"))
            })
            .await;

        assert!(result.is_err());
        assert!(runner.is_task_running(&task_id).await);

        let event = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("cancelled lifecycle event timeout")
            .expect("cancelled lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Cancelled { id, .. } if id == task_id
        ));
        assert!(!runner.is_task_running(&task_id).await);
    }

    #[tokio::test]
    async fn waiter_rechecks_claim_state_inside_the_registry_cleanup_boundary() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-atomic-claim".to_string();
        let child = spawn_success_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        let processes = runner.running_processes.lock().await;
        let termination_tx = processes
            .get(&task_id)
            .expect("registered process")
            .termination_tx
            .clone();
        runner.begin_wait_for_test(&task_id).await;

        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = termination_tx.send(ProcessTerminationState::Claimed);
        drop(processes);

        assert!(timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err());
        termination_tx
            .send(ProcessTerminationState::Committed)
            .expect("commit cancellation");

        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("cancelled lifecycle event timeout")
            .expect("cancelled lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Cancelled { id, .. } if id == task_id
        ));
    }

    #[tokio::test]
    async fn post_exit_claim_does_not_invoke_os_kill_for_a_reaped_pid() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-post-exit-claim".to_string();
        let child = spawn_success_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        let processes = runner.running_processes.lock().await;
        let process = processes.get(&task_id).expect("registered process").clone();
        let claim = TaskTerminationClaim {
            task_id: task_id.clone(),
            generation: process.generation,
        };
        runner.begin_wait_for_test(&task_id).await;

        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = process
            .termination_tx
            .send(ProcessTerminationState::Claimed);
        drop(processes);

        let kill_called = Arc::new(AtomicBool::new(false));
        let kill_called_by_request = Arc::clone(&kill_called);
        runner
            .terminate_claimed_task_with(&claim, move |_| async move {
                kill_called_by_request.store(true, Ordering::SeqCst);
                Ok(super::KillProcessResult::AlreadyExited)
            })
            .await
            .expect("complete already exited cancellation");

        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("cancelled lifecycle event timeout")
            .expect("cancelled lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Cancelled { id, .. } if id == task_id
        ));
        assert!(!kill_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn kill_future_timeout_is_reported_by_the_termination_layer() {
        let result = timeout(
            Duration::from_millis(200),
            super::run_kill_future_with_timeout(std::future::pending(), Duration::from_millis(25)),
        )
        .await
        .expect("termination layer should enforce its own timeout");

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn timed_out_os_helper_cannot_continue_after_timeout() {
        let marker = std::env::temp_dir().join(format!("kill-helper-marker-{}", Uuid::new_v4()));
        let mut command = delayed_marker_command(&marker);

        let result = super::run_command_output_with_timeout(
            &mut command,
            Duration::from_millis(150),
            "test OS helper",
        )
        .await;

        assert!(result.is_err());
        tokio::time::sleep(Duration::from_millis(900)).await;
        assert!(!marker.exists());
        let _ = fs::remove_file(marker);
    }

    #[test]
    fn process_group_poll_stops_when_its_deadline_expires() {
        let now = tokio::time::Instant::now();
        let deadline = now + Duration::from_millis(80);

        assert_eq!(
            super::remaining_until_deadline(deadline, now + Duration::from_millis(30)),
            Some(Duration::from_millis(50))
        );
        assert_eq!(
            super::remaining_until_deadline(deadline, deadline + Duration::from_millis(1)),
            None
        );
    }

    #[tokio::test]
    async fn reaped_termination_request_reports_cleanup_failure_for_retry() {
        let kill_called = Arc::new(AtomicBool::new(false));
        let kill_called_by_request = Arc::clone(&kill_called);
        let kill: super::KillProcessOperation = Box::new(move |_| {
            Box::pin(async move {
                kill_called_by_request.store(true, Ordering::SeqCst);
                Ok(super::KillProcessResult::Killed)
            }) as super::KillProcessFuture
        });
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = super::ProcessTerminationRequest { kill, response_tx };

        let completed =
            super::complete_reaped_termination_request_with(42, request, |pid, kill| async move {
                let _ = kill(pid).await?;
                Err(AppError::message("descendant still running"))
            })
            .await;

        assert!(completed.is_none());
        assert!(kill_called.load(Ordering::SeqCst));
        assert!(response_rx.await.expect("termination response").is_err());
    }

    #[tokio::test]
    async fn abort_does_not_downgrade_a_committed_cancellation() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-committed-cancel".to_string();
        let child = spawn_sleeping_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        runner.begin_wait_for_test(&task_id).await;

        let TaskTerminationClaimOutcome::Claimed(first_claim) = runner
            .claim_task_termination(&task_id)
            .await
            .expect("first claim")
        else {
            panic!("running child should be claimable");
        };
        runner
            .terminate_claimed_task_with(&first_claim, |_| async {
                Err(AppError::message("simulated permission denial"))
            })
            .await
            .expect_err("kill failure should be returned");

        let TaskTerminationClaimOutcome::Claimed(retry_claim) = runner
            .claim_task_termination(&task_id)
            .await
            .expect("retry claim")
        else {
            panic!("committed child should remain claimable for a kill retry");
        };
        runner.abort_task_termination(&retry_claim).await;

        let event = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("cancelled lifecycle event timeout")
            .expect("cancelled lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Cancelled { id, .. } if id == task_id
        ));
    }

    #[tokio::test]
    async fn a_duplicate_claim_cannot_abort_the_original_claim() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-duplicate-claim".to_string();
        let child = spawn_success_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        let TaskTerminationClaimOutcome::Claimed(original_claim) = runner
            .claim_task_termination(&task_id)
            .await
            .expect("original claim")
        else {
            panic!("running child should be claimable");
        };
        let duplicate_outcome = runner
            .claim_task_termination(&task_id)
            .await
            .expect("duplicate claim");
        if let TaskTerminationClaimOutcome::Claimed(duplicate_claim) = duplicate_outcome {
            runner.abort_task_termination(&duplicate_claim).await;
        }

        runner.begin_wait_for_test(&task_id).await;
        assert!(timeout(Duration::from_secs(1), rx.recv()).await.is_err());

        runner.abort_task_termination(&original_claim).await;
        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("natural lifecycle event timeout")
            .expect("natural lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Completed { id, .. } if id == task_id
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_termination_kills_descendants_after_group_leader_exits() {
        use tokio::process::Command;

        let mut command = Command::new("sh");
        command
            .args(["-c", "trap 'exit 0' TERM; (trap '' TERM; sleep 30) & wait"])
            .process_group(0);
        let mut child = command.spawn().expect("spawn process group test child");
        let pid = child.id().expect("process group leader pid");
        let kill: super::KillProcessOperation =
            Box::new(|pid| Box::pin(super::kill_process(pid)) as super::KillProcessFuture);

        let result = super::terminate_child_process(&mut child, "unix-process-group", kill).await;
        if result.is_err() {
            let _ = super::force_kill_process(pid).await;
            let _ = child.wait().await;
        }

        result.expect("terminate full Unix process group");
        assert!(!super::process_group_exists(pid)
            .await
            .expect("probe terminated process group"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn post_exit_claim_terminates_surviving_unix_process_group() {
        use tokio::process::Command;

        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = TaskRunner::with_lifecycle_sender(tx);
        let task_id = "task-reaped-unix-group".to_string();
        let mut command = Command::new("sh");
        command
            .args(["-c", "(trap '' HUP; sleep 30) & exit 0"])
            .process_group(0);
        let child = command.spawn().expect("spawn reaped group leader");
        let pid = child.id().expect("process group leader pid");

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        let TaskTerminationClaimOutcome::Claimed(claim) = runner
            .claim_task_termination(&task_id)
            .await
            .expect("claim reaped process group")
        else {
            panic!("process group should be claimable");
        };
        runner.begin_wait_for_test(&task_id).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(super::process_group_exists(pid)
            .await
            .expect("probe surviving descendant"));

        let kill_called = Arc::new(AtomicBool::new(false));
        let kill_called_by_request = Arc::clone(&kill_called);
        runner
            .terminate_claimed_task_with(&claim, move |pid| async move {
                kill_called_by_request.store(true, Ordering::SeqCst);
                super::kill_process(pid).await
            })
            .await
            .expect("terminate reaped process group");

        let event = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("cancelled lifecycle event timeout")
            .expect("cancelled lifecycle event");
        assert!(matches!(
            event,
            TaskLifecycleEvent::Cancelled { id, .. } if id == task_id
        ));
        assert!(kill_called.load(Ordering::SeqCst));
        assert!(!super::process_group_exists(pid)
            .await
            .expect("probe terminated process group"));
    }

    #[test]
    fn find_cli_in_ancestors_respects_search_depth() {
        let root = std::env::temp_dir().join(format!("cli-search-{}", Uuid::new_v4()));
        let nested = root.join("a").join("b").join("c").join("d");
        let cli_path = root.join("N_m3u8DL-CLI_v3.0.2.exe");

        fs::create_dir_all(&nested).expect("create nested dirs");
        fs::write(&cli_path, b"").expect("create fake cli");

        let found = find_cli_in_ancestors(&nested, "N_m3u8DL-CLI_v3.0.2.exe", 4);
        let missed = find_cli_in_ancestors(&nested, "N_m3u8DL-CLI_v3.0.2.exe", 3);

        assert_eq!(found, Some(cli_path));
        assert_eq!(missed, None);

        fs::remove_dir_all(&root).expect("cleanup temp dirs");
    }
}
