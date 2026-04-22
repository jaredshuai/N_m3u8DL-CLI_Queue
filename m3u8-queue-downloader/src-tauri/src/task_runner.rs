use crate::models::Task;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tauri::{path::BaseDirectory, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[derive(Clone)]
struct RunningTask {
    pid: u32,
    stop_state: Arc<AtomicU8>,
}

pub struct TaskRunner {
    running_processes: Arc<Mutex<HashMap<String, RunningTask>>>,
    #[cfg(test)]
    pending_test_children: Arc<Mutex<HashMap<String, Child>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopTaskError {
    KillFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopTaskResult {
    Stopped,
    AlreadyExited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KillProcessResult {
    Killed,
    AlreadyExited,
}

const MAX_CLI_SEARCH_DEPTH: usize = 8;
const STOP_NONE: u8 = 0;
const STOP_REQUESTED: u8 = 1;
const STOP_CONFIRMED: u8 = 2;
const STOP_REQUEST_GRACE_MS: u64 = 500;

impl fmt::Display for StopTaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

    pub async fn start_task(&self, task: Task, app_handle: tauri::AppHandle) -> Result<(), String> {
        let cli_path = self.find_cli_exe(&app_handle)?;

        let mut args: Vec<String> = Vec::new();
        args.push(task.url.clone());
        args.push("--workDir".to_string());
        args.push(r"D:\Videos".to_string());

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
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);

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
        let stop_state = Arc::new(AtomicU8::new(STOP_NONE));

        self.register_running_task(task_id.clone(), pid, Arc::clone(&stop_state))
            .await;

        let task_id_stdout = task_id.clone();
        let app_handle_stdout = app_handle_progress.clone();
        tokio::spawn(async move {
            read_cli_stdout(stdout, task_id_stdout, app_handle_stdout).await;
        });

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

        self.spawn_wait_task(task_id, child, stop_state, save_name, app_handle_complete);
        Ok(())
    }

    pub async fn stop_task(&self, task_id: &str) -> Result<StopTaskResult, StopTaskError> {
        let running_task = {
            let processes = self.running_processes.lock().await;
            processes.get(task_id).cloned()
        };

        let Some(running_task) = running_task else {
            return Ok(StopTaskResult::AlreadyExited);
        };

        running_task
            .stop_state
            .store(STOP_REQUESTED, Ordering::SeqCst);

        match kill_process(running_task.pid).await {
            Ok(KillProcessResult::Killed) => {
                running_task
                    .stop_state
                    .store(STOP_CONFIRMED, Ordering::SeqCst);
                Ok(StopTaskResult::Stopped)
            }
            Ok(KillProcessResult::AlreadyExited) => {
                running_task.stop_state.store(STOP_NONE, Ordering::SeqCst);
                Ok(StopTaskResult::AlreadyExited)
            }
            Err(err) => {
                running_task.stop_state.store(STOP_NONE, Ordering::SeqCst);
                Err(StopTaskError::KillFailed(err))
            }
        }
    }

    #[cfg(test)]
    pub async fn is_task_running(&self, task_id: &str) -> bool {
        let processes = self.running_processes.lock().await;
        processes.contains_key(task_id)
    }

    async fn register_running_task(&self, task_id: String, pid: u32, stop_state: Arc<AtomicU8>) {
        let mut processes = self.running_processes.lock().await;
        processes.insert(task_id, RunningTask { pid, stop_state });
    }

    fn spawn_wait_task(
        &self,
        task_id: String,
        mut child: Child,
        stop_state: Arc<AtomicU8>,
        save_name: Option<String>,
        app_handle: tauri::AppHandle,
    ) {
        let running_processes = Arc::clone(&self.running_processes);

        tokio::spawn(async move {
            let result = child.wait().await;
            cleanup_running_task(&running_processes, &task_id).await;

            if wait_for_confirmed_stop(&stop_state).await {
                return;
            }

            match result {
                Ok(exit_status) => {
                    if exit_status.success() {
                        let output_path = find_output_file(&save_name);
                        let payload = serde_json::json!({
                            "id": task_id,
                            "outputPath": output_path.unwrap_or_default(),
                        });
                        let _ = app_handle.emit("task-completed", payload);
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
        let stop_state = Arc::new(AtomicU8::new(STOP_NONE));

        self.register_running_task(task_id.clone(), pid, Arc::clone(&stop_state))
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
        let stop_state = {
            let processes = self.running_processes.lock().await;
            Arc::clone(&processes.get(task_id).expect("running task").stop_state)
        };
        let running_processes = Arc::clone(&self.running_processes);
        let task_id = task_id.to_string();

        tokio::spawn(async move {
            let mut child = child;
            let _ = child.wait().await;
            cleanup_running_task(&running_processes, &task_id).await;
            let _ = wait_for_confirmed_stop(&stop_state).await;
        });
    }

    fn find_cli_exe(&self, app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
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

        Err(format!(
            "{} not found in bundled resources or any searched directory",
            cli_name
        ))
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

async fn wait_for_confirmed_stop(stop_state: &Arc<AtomicU8>) -> bool {
    let mut waited = 0;
    loop {
        match stop_state.load(Ordering::SeqCst) {
            STOP_CONFIRMED => return true,
            STOP_REQUESTED if waited < STOP_REQUEST_GRACE_MS => {
                sleep(Duration::from_millis(10)).await;
                waited += 10;
            }
            _ => return false,
        }
    }
}

async fn read_cli_stdout<R>(mut stdout: R, task_id: String, app_handle: tauri::AppHandle)
where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 4096];
    let mut pending = String::new();

    loop {
        match stdout.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                pending.push_str(&String::from_utf8_lossy(&buffer[..n]));
                while let Some((segment, rest)) = take_cli_segment(&pending) {
                    pending = rest;
                    emit_cli_segment(&segment, &task_id, &app_handle);
                }
            }
            Err(_) => break,
        }
    }

    let trailing = pending.trim();
    if !trailing.is_empty() {
        emit_cli_segment(trailing, &task_id, &app_handle);
    }
}

fn take_cli_segment(input: &str) -> Option<(String, String)> {
    let split_at = input.find(|ch| ch == '\r' || ch == '\n')?;
    let segment = input[..split_at].trim().to_string();
    let delimiter_len = input[split_at..].chars().next()?.len_utf8();
    let mut rest_start = split_at + delimiter_len;
    if input[split_at..].starts_with("\r\n") {
        rest_start = split_at + 2;
    }
    let rest = input[rest_start..].to_string();
    Some((segment, rest))
}

fn emit_cli_segment(segment: &str, task_id: &str, app_handle: &tauri::AppHandle) {
    if segment.is_empty() {
        return;
    }

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

    let log_payload = serde_json::json!({
        "id": task_id,
        "line": segment,
    });
    let _ = app_handle.emit("task-log", log_payload);
}

fn parse_progress(line: &str) -> Option<f32> {
    if let Some(caps) = regex_progress_percent().captures(line) {
        if let Some(m) = caps.get(1) {
            let normalized = m.as_str().replace(',', ".");
            if let Ok(pct) = normalized.parse::<f32>() {
                return Some((pct / 100.0).clamp(0.0, 1.0));
            }
        }
    }

    if let Some(caps) = regex_progress_count().captures(line) {
        let current = caps.get(1)?.as_str().parse::<f32>().ok()?;
        let total = caps.get(2)?.as_str().parse::<f32>().ok()?;
        if total > 0.0 {
            return Some((current / total).clamp(0.0, 1.0));
        }
    }
    None
}

fn parse_speed(line: &str) -> Option<String> {
    if let Some(caps) = regex_speed_lazy().captures(line) {
        if let Some(m) = caps.get(1) {
            return Some(m.as_str().trim().to_string());
        }
    }
    None
}

fn parse_threads(line: &str) -> Option<String> {
    if let Some(caps) = regex_threads_lazy().captures(line) {
        if let Some(m) = caps.get(1).or_else(|| caps.get(2)) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

fn find_output_file(save_name: &Option<String>) -> Option<String> {
    let output_dir = PathBuf::from(r"D:\Videos");

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

use regex::{Regex, RegexBuilder};
use std::sync::OnceLock;

static PROGRESS_PERCENT_RE: OnceLock<Regex> = OnceLock::new();
static PROGRESS_COUNT_RE: OnceLock<Regex> = OnceLock::new();
static SPEED_RE: OnceLock<Regex> = OnceLock::new();
static THREADS_RE: OnceLock<Regex> = OnceLock::new();

fn regex_progress_percent() -> &'static Regex {
    PROGRESS_PERCENT_RE.get_or_init(|| Regex::new(r"([0-9]+(?:[\.,][0-9]+)?)%").unwrap())
}

fn regex_progress_count() -> &'static Regex {
    PROGRESS_COUNT_RE.get_or_init(|| Regex::new(r"Progress:\s*(\d+)\s*/\s*(\d+)").unwrap())
}

fn regex_speed_lazy() -> &'static Regex {
    SPEED_RE.get_or_init(|| {
        RegexBuilder::new(r"([0-9]+(?:[\.,][0-9]+)?\s*(?:[KMGT]i?B|KB|MB|GB|TB|B)/(?:s|秒))")
            .case_insensitive(true)
            .build()
            .unwrap()
    })
}

fn regex_threads_lazy() -> &'static Regex {
    THREADS_RE.get_or_init(|| {
        RegexBuilder::new(r"(?:threads?|线程(?:数)?)[\s:=：]*(\d+)|([0-9]+)[\s]*(?:threads?|线程)")
            .case_insensitive(true)
            .build()
            .unwrap()
    })
}

async fn cleanup_running_task(
    running_processes: &Arc<Mutex<HashMap<String, RunningTask>>>,
    task_id: &str,
) {
    let mut processes = running_processes.lock().await;
    processes.remove(task_id);
}

#[cfg(target_os = "windows")]
async fn kill_process(pid: u32) -> Result<KillProcessResult, String> {
    let mut cmd = tokio::process::Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to launch taskkill: {}", e))?;

    if output.status.success() {
        return Ok(KillProcessResult::Killed);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    let combined = format!("{stdout}\n{stderr}");
    if combined.contains("not found")
        || combined.contains("no running instance")
        || combined.contains("没有找到")
        || combined.contains("找不到")
    {
        return Ok(KillProcessResult::AlreadyExited);
    }

    Err(format!(
        "taskkill exited with code {}: {}",
        output.status.code().unwrap_or(-1),
        combined.trim()
    ))
}

#[cfg(not(target_os = "windows"))]
async fn kill_process(pid: u32) -> Result<KillProcessResult, String> {
    let output = tokio::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to launch kill: {}", e))?;

    if output.status.success() {
        return Ok(KillProcessResult::Killed);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("no such process") {
        return Ok(KillProcessResult::AlreadyExited);
    }

    Err(format!(
        "kill exited with code {}: {}",
        output.status.code().unwrap_or(-1),
        stderr.trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        find_cli_in_ancestors, parse_progress, parse_speed, parse_threads, take_cli_segment,
        TaskRunner,
    };
    use crate::test_support::spawn_sleeping_child;
    use std::fs;
    use tokio::time::{sleep, Duration};
    use uuid::Uuid;

    #[test]
    fn parse_progress_reads_cli_percent_as_ratio() {
        assert_eq!(
            parse_progress("Progress: 10/40 (25.00%) -- 1.0MB/4.0MB"),
            Some(0.25)
        );
        assert_eq!(parse_progress("speed only 1.5 MB/s"), None);
    }

    #[test]
    fn parse_speed_reads_cli_speed_units() {
        assert_eq!(
            parse_speed("(1.5 MB/s @ 00:01:00)").as_deref(),
            Some("1.5 MB/s")
        );
    }

    #[test]
    fn parse_progress_accepts_comma_decimal_and_count_fallback() {
        assert_eq!(
            parse_progress("Progress: 1/4 (25,00%) -- 1MB/4MB"),
            Some(0.25)
        );
        assert_eq!(parse_progress("Progress: 2/8 -- 1MB/4MB"), Some(0.25));
    }

    #[test]
    fn parse_threads_reads_common_cli_formats() {
        assert_eq!(parse_threads("Threads: 16").as_deref(), Some("16"));
        assert_eq!(parse_threads("16 threads active").as_deref(), Some("16"));
    }

    #[test]
    fn take_cli_segment_splits_on_carriage_return() {
        let (segment, rest) = take_cli_segment("Progress: 1/2 (50.00%)\rnext").expect("segment");
        assert_eq!(segment, "Progress: 1/2 (50.00%)");
        assert_eq!(rest, "next");
    }

    #[tokio::test]
    async fn stop_task_is_idempotent_after_process_already_exited() {
        let runner = TaskRunner::new();

        runner
            .stop_task("missing-task")
            .await
            .expect("stop on missing process should not error");
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

        let found = find_cli_in_ancestors(&nested, "N_m3u8DL-CLI_v3.0.2.exe", 4);
        let missed = find_cli_in_ancestors(&nested, "N_m3u8DL-CLI_v3.0.2.exe", 3);

        assert_eq!(found, Some(cli_path));
        assert_eq!(missed, None);

        fs::remove_dir_all(&root).expect("cleanup temp dirs");
    }
}
