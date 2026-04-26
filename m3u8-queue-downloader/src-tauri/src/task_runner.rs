use crate::app_error::{AppError, AppResult};
use crate::models::Task;
use crate::progress_parser::{parse_progress, parse_speed, parse_threads};
use crate::terminal_parser::{decode_cli_bytes_lossy, TerminalBuffer};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashMap as StdHashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{path::BaseDirectory, Emitter, Manager};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

pub struct TaskRunner {
    running_processes: Arc<Mutex<StdHashMap<String, u32>>>,
    lifecycle_sender: Option<mpsc::UnboundedSender<TaskLifecycleEvent>>,
    #[cfg(test)]
    pending_test_children: Arc<Mutex<HashMap<String, Child>>>,
}

const MAX_CLI_SEARCH_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskLifecycleEvent {
    Completed { id: String, output_path: String },
    Failed { id: String, error_message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KillProcessResult {
    Killed,
    AlreadyExited,
}

impl TaskRunner {
    pub fn new() -> Self {
        Self {
            running_processes: Arc::new(Mutex::new(StdHashMap::new())),
            lifecycle_sender: None,
            #[cfg(test)]
            pending_test_children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_lifecycle_sender(sender: mpsc::UnboundedSender<TaskLifecycleEvent>) -> Self {
        Self {
            running_processes: Arc::new(Mutex::new(StdHashMap::new())),
            lifecycle_sender: Some(sender),
            #[cfg(test)]
            pending_test_children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn start_task(
        &self,
        task: Task,
        download_dir: PathBuf,
        app_handle: tauri::AppHandle,
    ) -> AppResult<()> {
        let cli_path = self.find_cli_exe(&app_handle)?;

        let mut args: Vec<String> = Vec::new();
        args.push(task.url.clone());
        args.push("--workDir".to_string());
        args.push(download_dir.to_string_lossy().to_string());

        if let Some(ref save_name) = task.save_name {
            if !save_name.is_empty() {
                args.push("--saveName".to_string());
                args.push(save_name.clone());
            }
        }

        if let Some(ref headers) = task.headers {
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

        let task_id = task.id.clone();
        let save_name = task.save_name.clone();
        let app_handle_progress = app_handle.clone();
        let pid = child
            .id()
            .ok_or_else(|| AppError::message("Failed to get CLI process ID"))?;
        self.register_running_task(task_id.clone(), pid).await;

        let task_id_stdout = task_id.clone();
        let app_handle_stdout = app_handle_progress.clone();
        tokio::spawn(async move {
            read_cli_stream(stdout, task_id_stdout, app_handle_stdout, None).await;
        });

        let task_id_stderr = task_id.clone();
        let app_handle_stderr = app_handle_progress.clone();
        tokio::spawn(async move {
            read_cli_stream(stderr, task_id_stderr, app_handle_stderr, Some("[stderr] ")).await;
        });

        self.spawn_wait_task(task_id, child, save_name, download_dir);
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
                .map(|(task_id, pid)| (task_id.clone(), *pid))
                .collect::<Vec<_>>()
        };

        for (task_id, pid) in &running {
            match kill_process(*pid).await {
                Ok(KillProcessResult::Killed) | Ok(KillProcessResult::AlreadyExited) => {}
                Err(err) => {
                    return Err(AppError::message(format!(
                        "Failed to terminate task {task_id}: {err}"
                    )));
                }
            }
        }

        let mut processes = self.running_processes.lock().await;
        for (task_id, _) in &running {
            processes.remove(task_id);
        }

        #[cfg(test)]
        {
            let mut pending = self.pending_test_children.lock().await;
            for (task_id, _) in &running {
                pending.remove(task_id);
            }
        }

        Ok(())
    }

    async fn register_running_task(&self, task_id: String, pid: u32) {
        let mut processes = self.running_processes.lock().await;
        processes.insert(task_id, pid);
    }

    fn spawn_wait_task(
        &self,
        task_id: String,
        mut child: Child,
        save_name: Option<String>,
        download_dir: PathBuf,
    ) {
        let running_processes = Arc::clone(&self.running_processes);
        let lifecycle_sender = self.lifecycle_sender.clone();

        tokio::spawn(async move {
            let result = child.wait().await;
            cleanup_running_task(&running_processes, &task_id).await;

            let event = match result {
                Ok(exit_status) => {
                    if exit_status.success() {
                        let output_path = find_output_file(&download_dir, &save_name);
                        TaskLifecycleEvent::Completed {
                            id: task_id,
                            output_path: output_path.unwrap_or_default(),
                        }
                    } else {
                        let error_msg = format!(
                            "Process exited with code: {}",
                            exit_status.code().unwrap_or(-1)
                        );
                        TaskLifecycleEvent::Failed {
                            id: task_id,
                            error_message: error_msg,
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("Process error: {}", e);
                    TaskLifecycleEvent::Failed {
                        id: task_id,
                        error_message: error_msg,
                    }
                }
            };

            if let Some(sender) = lifecycle_sender {
                let _ = sender.send(event);
            }
        });
    }

    #[cfg(test)]
    pub(crate) async fn insert_running_task_for_test(&self, task_id: String, child: Child) {
        let pid = child.id().expect("test child pid");
        self.register_running_task(task_id.clone(), pid).await;

        let mut pending = self.pending_test_children.lock().await;
        pending.insert(task_id, child);
    }

    #[cfg(test)]
    pub(crate) async fn begin_wait_for_test(&self, task_id: &str) {
        let child = {
            let mut pending = self.pending_test_children.lock().await;
            pending.remove(task_id).expect("pending test child")
        };
        let running_processes = Arc::clone(&self.running_processes);
        let lifecycle_sender = self.lifecycle_sender.clone();
        let task_id = task_id.to_string();

        tokio::spawn(async move {
            let mut child = child;
            let result = child.wait().await;
            cleanup_running_task(&running_processes, &task_id).await;
            if let Some(sender) = lifecycle_sender {
                let event = match result {
                    Ok(status) if status.success() => TaskLifecycleEvent::Completed {
                        id: task_id,
                        output_path: String::new(),
                    },
                    Ok(status) => TaskLifecycleEvent::Failed {
                        id: task_id,
                        error_message: format!(
                            "Process exited with code: {}",
                            status.code().unwrap_or(-1)
                        ),
                    },
                    Err(err) => TaskLifecycleEvent::Failed {
                        id: task_id,
                        error_message: format!("Process error: {err}"),
                    },
                };
                let _ = sender.send(event);
            }
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
    app_handle: tauri::AppHandle,
    log_prefix: Option<&'static str>,
) where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 4096];
    let mut term = TerminalBuffer::new();

    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                term.feed(&buffer[..n]);

                for line in term.take_committed() {
                    emit_committed_line(&line, &task_id, &app_handle, log_prefix);
                }

                if should_emit_active_line(log_prefix) {
                    let active = term.active_line().trim().to_string();
                    emit_active_line(&active, &task_id, &app_handle, log_prefix);
                }
            }
            Err(_) => break,
        }
    }

    term.finish();
    for line in term.take_committed() {
        emit_committed_line(&line, &task_id, &app_handle, log_prefix);
    }
    if should_emit_active_line(log_prefix) {
        emit_active_line("", &task_id, &app_handle, log_prefix);
    }
}

fn emit_committed_line(
    segment: &str,
    task_id: &str,
    app_handle: &tauri::AppHandle,
    log_prefix: Option<&str>,
) {
    if segment.is_empty() {
        return;
    }

    if log_prefix.is_none() {
        let progress = parse_progress(segment);
        let speed = parse_speed(segment);
        let threads = parse_threads(segment);

        if progress.is_some() || speed.is_some() || threads.is_some() {
            let payload = serde_json::json!({
                "id": task_id,
                "progress": progress,
                "speed": speed,
                "threads": threads,
            });
            let _ = app_handle.emit("task-progress", payload);
        }
    }

    let line = if let Some(prefix) = log_prefix {
        format!("{prefix}{segment}")
    } else {
        segment.to_string()
    };

    let log_payload = serde_json::json!({
        "id": task_id,
        "line": line,
    });
    let _ = app_handle.emit("task-log", log_payload);

    let terminal_payload = serde_json::json!({
        "id": task_id,
        "line": line,
    });
    let _ = app_handle.emit("task-terminal-committed-line", terminal_payload);
}

fn should_emit_active_line(log_prefix: Option<&str>) -> bool {
    log_prefix.is_none()
}

fn emit_active_line(
    line: &str,
    task_id: &str,
    app_handle: &tauri::AppHandle,
    log_prefix: Option<&str>,
) {
    if !line.is_empty() && log_prefix.is_none() {
        let progress = parse_progress(line);
        let speed = parse_speed(line);
        let threads = parse_threads(line);

        if progress.is_some() || speed.is_some() || threads.is_some() {
            let payload = serde_json::json!({
                "id": task_id,
                "progress": progress,
                "speed": speed,
                "threads": threads,
            });
            let _ = app_handle.emit("task-progress", payload);
        }
    }

    let prefixed = if line.is_empty() {
        String::new()
    } else if let Some(prefix) = log_prefix {
        format!("{prefix}{line}")
    } else {
        line.to_string()
    };

    let payload = serde_json::json!({
        "id": task_id,
        "activeLine": prefixed,
    });
    let _ = app_handle.emit("task-terminal-active-line", payload);
}

fn find_output_file(output_dir: &PathBuf, save_name: &Option<String>) -> Option<String> {
    if !output_dir.exists() {
        return None;
    }

    let extensions = ["mp4", "mkv", "ts", "flv", "mpg", "mpeg"];

    if let Some(ref name) = save_name {
        if !name.is_empty() {
            for ext in &extensions {
                let exact = output_dir.join(format!("{}.{}", name, ext));
                if exact.exists() {
                    return exact.to_str().map(|s| s.to_string());
                }
            }

            if let Ok(entries) = std::fs::read_dir(&output_dir) {
                let mut matching: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let file_name = e.file_name();
                        let file_name_str = file_name.to_string_lossy();
                        file_name_str.starts_with(name)
                            && extensions
                                .iter()
                                .any(|ext| file_name_str.ends_with(&format!(".{}", ext)))
                    })
                    .collect();

                matching.sort_by(|a, b| {
                    let a_time = a.metadata().ok().and_then(|m| m.modified().ok());
                    let b_time = b.metadata().ok().and_then(|m| m.modified().ok());
                    b_time.cmp(&a_time)
                });

                if let Some(first) = matching.first() {
                    return first.path().to_str().map(|s| s.to_string());
                }
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir(&output_dir) {
        let now = std::time::SystemTime::now();
        let mut recent: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let file_name = e.file_name();
                let file_name_str = file_name.to_string_lossy();
                extensions
                    .iter()
                    .any(|ext| file_name_str.ends_with(&format!(".{}", ext)))
            })
            .filter(|e| {
                e.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|mod_time| {
                        now.duration_since(mod_time)
                            .unwrap_or(std::time::Duration::ZERO)
                            < std::time::Duration::from_secs(60)
                    })
                    .unwrap_or(false)
            })
            .collect();

        recent.sort_by(|a, b| {
            let a_time = a.metadata().ok().and_then(|m| m.modified().ok());
            let b_time = b.metadata().ok().and_then(|m| m.modified().ok());
            b_time.cmp(&a_time)
        });

        if let Some(first) = recent.first() {
            return first.path().to_str().map(|s| s.to_string());
        }
    }

    None
}

async fn cleanup_running_task(
    running_processes: &Arc<Mutex<StdHashMap<String, u32>>>,
    task_id: &str,
) {
    let mut processes = running_processes.lock().await;
    processes.remove(task_id);
}

#[cfg(target_os = "windows")]
async fn kill_process(pid: u32) -> AppResult<KillProcessResult> {
    let mut cmd = tokio::process::Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::message(format!("Failed to launch taskkill: {}", e)))?;

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

#[cfg(not(target_os = "windows"))]
async fn kill_process(pid: u32) -> AppResult<KillProcessResult> {
    let output = tokio::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| AppError::message(format!("Failed to launch kill: {}", e)))?;

    if output.status.success() {
        return Ok(KillProcessResult::Killed);
    }

    let stderr = decode_cli_bytes_lossy(&output.stderr).to_lowercase();
    if stderr.contains("no such process") {
        return Ok(KillProcessResult::AlreadyExited);
    }

    Err(AppError::message(format!(
        "kill exited with code {}: {}",
        output.status.code().unwrap_or(-1),
        stderr.trim()
    )))
}

#[cfg(test)]
mod tests {
    use super::{find_cli_in_ancestors, should_emit_active_line, TaskLifecycleEvent, TaskRunner};
    use crate::test_support::{spawn_sleeping_child, spawn_success_child};
    use std::fs;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio::time::timeout;
    use uuid::Uuid;

    #[test]
    fn stderr_stream_does_not_drive_terminal_active_line() {
        assert!(should_emit_active_line(None));
        assert!(!should_emit_active_line(Some("[stderr] ")));
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

        runner
            .terminate_all_running_processes()
            .await
            .expect("terminate running processes");

        assert!(!runner.is_task_running(&task_id).await);
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
