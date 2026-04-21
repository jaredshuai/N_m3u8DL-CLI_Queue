use crate::models::Task;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::Mutex;

#[derive(Clone)]
struct RunningTask {
    pid: u32,
    stopped: Arc<AtomicBool>,
}

/// Manages the execution of CLI download processes
pub struct TaskRunner {
    /// Map from task ID to running process metadata
    running_processes: Arc<Mutex<HashMap<String, RunningTask>>>,
    #[cfg(test)]
    pending_test_children: Arc<Mutex<HashMap<String, Child>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopTaskError {
    NoRunningProcess(String),
    KillFailed(String),
}

const MAX_CLI_SEARCH_DEPTH: usize = 8;

impl fmt::Display for StopTaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StopTaskError::NoRunningProcess(task_id) => {
                write!(f, "No running process for task {task_id}")
            }
            StopTaskError::KillFailed(message) => f.write_str(message),
        }
    }
}

impl TaskRunner {
    pub fn new() -> Self {
        Self {
            running_processes: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(test)]
            pending_test_children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a CLI process for the given task
    pub async fn start_task(&self, task: Task, app_handle: tauri::AppHandle) -> Result<(), String> {
        let cli_path = self.find_cli_exe()?;

        // Build command arguments
        let mut args: Vec<String> = Vec::new();

        // URL
        args.push(task.url.clone());

        // workDir
        args.push("--workDir".to_string());
        args.push(r"D:\Videos".to_string());

        // saveName
        if let Some(ref save_name) = task.save_name {
            if !save_name.is_empty() {
                args.push("--saveName".to_string());
                args.push(save_name.clone());
            }
        }

        // headers
        if let Some(ref headers) = task.headers {
            if !headers.is_empty() {
                args.push("--headers".to_string());
                args.push(headers.clone());
            }
        }

        // enableDelAfterDone
        args.push("--enableDelAfterDone".to_string());

        let mut cmd = tokio::process::Command::new(&cli_path);
        cmd.args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Hide the console window on Windows
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn CLI process: {}", e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Failed to capture stderr".to_string())?;

        let task_id = task.id.clone();
        let save_name = task.save_name.clone();
        let app_handle_progress = app_handle.clone();
        let app_handle_complete = app_handle.clone();
        let pid = child
            .id()
            .ok_or_else(|| "Failed to get CLI process ID".to_string())?;
        let stopped = Arc::new(AtomicBool::new(false));

        self.register_running_task(task_id.clone(), pid, Arc::clone(&stopped))
            .await;

        // Spawn task to read stdout and parse progress
        let task_id_stdout = task_id.clone();
        let app_handle_stdout = app_handle_progress.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let line_to_log = line.clone();

                // Parse progress percentage
                let progress = parse_progress(&line);

                // Parse speed
                let speed = parse_speed(&line);

                // Parse threads info
                let threads = parse_threads(&line);

                // Emit task-progress event
                let payload = serde_json::json!({
                    "id": task_id_stdout,
                    "progress": progress,
                    "speed": speed,
                    "threads": threads,
                });
                let _ = app_handle_stdout.emit("task-progress", payload);

                // Append log line via event
                let log_payload = serde_json::json!({
                    "id": task_id_stdout,
                    "line": line_to_log,
                });
                let _ = app_handle_stdout.emit("task-log", log_payload);
            }
        });

        // Spawn task to read stderr
        let task_id_stderr = task_id.clone();
        let app_handle_stderr = app_handle_progress.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let log_payload = serde_json::json!({
                    "id": task_id_stderr,
                    "line": format!("[stderr] {}", line),
                });
                let _ = app_handle_stderr.emit("task-log", log_payload);
            }
        });

        // Spawn task to wait for process completion
        self.spawn_wait_task(task_id, child, stopped, save_name, app_handle_complete);

        Ok(())
    }

    /// Stop (kill) a running task's process
    pub async fn stop_task(&self, task_id: &str) -> Result<(), StopTaskError> {
        let running_task = {
            let processes = self.running_processes.lock().await;
            processes.get(task_id).cloned()
        }
        .ok_or_else(|| StopTaskError::NoRunningProcess(task_id.to_string()))?;

        kill_process(running_task.pid).await.map_err(StopTaskError::KillFailed)?;
        running_task.stopped.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Check if a specific task is running
    #[cfg(test)]
    pub async fn is_task_running(&self, task_id: &str) -> bool {
        let processes = self.running_processes.lock().await;
        processes.contains_key(task_id)
    }

    async fn register_running_task(&self, task_id: String, pid: u32, stopped: Arc<AtomicBool>) {
        let mut processes = self.running_processes.lock().await;
        processes.insert(task_id, RunningTask { pid, stopped });
    }

    fn spawn_wait_task(
        &self,
        task_id: String,
        mut child: Child,
        stopped: Arc<AtomicBool>,
        save_name: Option<String>,
        app_handle: tauri::AppHandle,
    ) {
        let running_processes = Arc::clone(&self.running_processes);

        tokio::spawn(async move {
            let result = child.wait().await;
            cleanup_running_task(&running_processes, &task_id).await;

            if stopped.load(Ordering::SeqCst) {
                return;
            }

            match result {
                Ok(exit_status) => {
                    if exit_status.success() {
                        // Try to find the output file
                        let output_path = find_output_file(&save_name);

                        if let Some(path) = output_path {
                            let payload = serde_json::json!({
                                "id": task_id,
                                "outputPath": path,
                            });
                            let _ = app_handle.emit("task-completed", payload);
                        } else {
                            // CLI succeeded but we couldn't find the file
                            // Still mark as completed, just without outputPath
                            let payload = serde_json::json!({
                                "id": task_id,
                                "outputPath": "",
                            });
                            let _ = app_handle.emit("task-completed", payload);
                        }
                    } else {
                        let error_msg = format!(
                            "Process exited with code: {}",
                            exit_status.code().unwrap_or(-1)
                        );
                        let payload = serde_json::json!({
                            "id": task_id,
                            "errorMessage": error_msg,
                        });
                        let _ = app_handle.emit("task-failed", payload);
                    }
                }
                Err(e) => {
                    let error_msg = format!("Process error: {}", e);
                    let payload = serde_json::json!({
                        "id": task_id,
                        "errorMessage": error_msg,
                    });
                    let _ = app_handle.emit("task-failed", payload);
                }
            }
        });
    }

    #[cfg(test)]
    pub(crate) async fn insert_running_task_for_test(&self, task_id: String, child: Child) {
        let pid = child.id().expect("test child pid");
        let stopped = Arc::new(AtomicBool::new(false));

        self.register_running_task(task_id.clone(), pid, stopped)
            .await;

        let mut pending = self.pending_test_children.lock().await;
        pending.insert(task_id, child);
    }

    #[cfg(test)]
    pub(crate) async fn begin_wait_for_test(&self, task_id: &str) {
        let child = {
            let mut pending = self.pending_test_children.lock().await;
            pending.remove(task_id).expect("pending test child")
        };
        let stopped = {
            let processes = self.running_processes.lock().await;
            Arc::clone(&processes.get(task_id).expect("running task").stopped)
        };
        let running_processes = Arc::clone(&self.running_processes);
        let task_id = task_id.to_string();

        tokio::spawn(async move {
            let mut child = child;
            let _ = child.wait().await;
            cleanup_running_task(&running_processes, &task_id).await;
            let _ = stopped;
        });
    }

    /// Find the N_m3u8DL-CLI executable by searching:
    /// 1. Same directory as the app executable
    /// 2. Walk up parent directories (for dev mode where exe is in target/debug/)
    /// 3. Current working directory
    fn find_cli_exe(&self) -> Result<PathBuf, String> {
        let cli_name = "N_m3u8DL-CLI_v3.0.2.exe";

        // 1. Check exe directory and its ancestors
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                if let Some(path) = find_cli_in_ancestors(exe_dir, cli_name, MAX_CLI_SEARCH_DEPTH)
                {
                    return Ok(path);
                }
            }
        }

        // 2. Check current working directory
        if let Ok(cwd) = std::env::current_dir() {
            let candidate = cwd.join(cli_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        Err(format!("{} not found in any searched directory", cli_name))
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

/// Parse progress percentage from CLI output line
fn parse_progress(line: &str) -> f32 {
    // Match patterns like "45.2%" or "100%"
    let re = regex_lazy();
    if let Some(caps) = re.captures(line) {
        if let Some(m) = caps.get(1) {
            if let Ok(pct) = m.as_str().parse::<f32>() {
                return pct / 100.0;
            }
        }
    }
    -1.0 // indicates no progress info in this line
}

/// Parse download speed from CLI output line
fn parse_speed(line: &str) -> String {
    // Match patterns like "1.5 MB/s", "500 KB/s", "2.3GB/s", etc.
    // Also handle Chinese output: "1.5 MB/秒"
    let patterns = [regex_speed_lazy()];

    for re in &patterns {
        if let Some(caps) = re.captures(line) {
            if let Some(m) = caps.get(1) {
                return m.as_str().to_string();
            }
        }
    }

    String::new()
}

/// Parse thread count from CLI output
fn parse_threads(line: &str) -> String {
    // Look for thread info pattern
    let re = regex_threads_lazy();
    if let Some(caps) = re.captures(line) {
        if let Some(m) = caps.get(0) {
            return m.as_str().to_string();
        }
    }
    String::new()
}

/// Find the output file after download completes
fn find_output_file(save_name: &Option<String>) -> Option<String> {
    let output_dir = PathBuf::from(r"D:\Videos");

    if !output_dir.exists() {
        return None;
    }

    let extensions = ["mp4", "mkv", "ts", "flv", "mpg", "mpeg"];

    // If save_name is specified, look for files starting with that name
    if let Some(ref name) = save_name {
        if !name.is_empty() {
            for ext in &extensions {
                // Exact match
                let exact = output_dir.join(format!("{}.{}", name, ext));
                if exact.exists() {
                    return exact.to_str().map(|s| s.to_string());
                }
            }

            // Prefix match (CLI sometimes appends suffixes)
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

                // Sort by modification time, most recent first
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

    // Fallback: look for recently modified files (within last 60 seconds)
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

use regex::Regex;
use std::sync::OnceLock;

static PROGRESS_RE: OnceLock<Regex> = OnceLock::new();
static SPEED_RE: OnceLock<Regex> = OnceLock::new();
static THREADS_RE: OnceLock<Regex> = OnceLock::new();

fn regex_lazy() -> &'static Regex {
    PROGRESS_RE.get_or_init(|| Regex::new(r"(\d+\.?\d*)%").unwrap())
}

fn regex_speed_lazy() -> &'static Regex {
    SPEED_RE
        .get_or_init(|| Regex::new(r"(\d+\.?\d*\s*[KMGT]?B/s|\d+\.?\d*\s*[KMGT]?B/秒)").unwrap())
}

fn regex_threads_lazy() -> &'static Regex {
    THREADS_RE.get_or_init(|| Regex::new(r"\d+\s*(?:线程|threads?)").unwrap())
}

async fn cleanup_running_task(
    running_processes: &Arc<Mutex<HashMap<String, RunningTask>>>,
    task_id: &str,
) {
    let mut processes = running_processes.lock().await;
    processes.remove(task_id);
}

#[cfg(target_os = "windows")]
async fn kill_process(pid: u32) -> Result<(), String> {
    let mut cmd = tokio::process::Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let status = cmd
        .status()
        .await
        .map_err(|e| format!("Failed to launch taskkill: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "taskkill exited with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}

#[cfg(not(target_os = "windows"))]
async fn kill_process(pid: u32) -> Result<(), String> {
    let status = tokio::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .await
        .map_err(|e| format!("Failed to launch kill: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "kill exited with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{find_cli_in_ancestors, TaskRunner};
    use crate::test_support::spawn_sleeping_child;
    use std::fs;
    use tokio::time::{sleep, Duration};
    use uuid::Uuid;

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
    async fn stop_task_kills_a_registered_process() {
        let runner = TaskRunner::new();
        let task_id = "task-2".to_string();
        let child = spawn_sleeping_child().await;

        runner
            .insert_running_task_for_test(task_id.clone(), child)
            .await;
        runner.begin_wait_for_test(&task_id).await;

        runner.stop_task(&task_id).await.expect("stop task");
        sleep(Duration::from_millis(200)).await;

        assert!(!runner.is_task_running(&task_id).await);
    }

    #[test]
    fn find_cli_in_ancestors_respects_search_depth() {
        let root = std::env::temp_dir().join(format!("cli-search-{}", Uuid::new_v4()));
        let nested = root.join("a").join("b").join("c").join("d");
        let cli_path = root.join("N_m3u8DL-CLI_v3.0.2.exe");

        fs::create_dir_all(&nested).expect("create nested dirs");
        fs::write(&cli_path, b"").expect("create fake cli");

        let found = find_cli_in_ancestors(
            &nested,
            "N_m3u8DL-CLI_v3.0.2.exe",
            4,
        );
        let missed = find_cli_in_ancestors(
            &nested,
            "N_m3u8DL-CLI_v3.0.2.exe",
            3,
        );

        assert_eq!(found, Some(cli_path));
        assert_eq!(missed, None);

        fs::remove_dir_all(&root).expect("cleanup temp dirs");
    }
}
