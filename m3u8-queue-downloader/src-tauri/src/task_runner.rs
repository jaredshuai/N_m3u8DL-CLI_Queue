use crate::models::Task;
use encoding_rs::{Encoding, GB18030};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tauri::{path::BaseDirectory, Emitter, Manager};
use tokio::io::{AsyncRead, AsyncReadExt};
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
            read_cli_stream(stdout, task_id_stdout, app_handle_stdout, None).await;
        });

        let task_id_stderr = task_id.clone();
        let app_handle_stderr = app_handle_progress.clone();
        tokio::spawn(async move {
            read_cli_stream(stderr, task_id_stderr, app_handle_stderr, Some("[stderr] ")).await;
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

struct TerminalBuffer {
    committed_lines: Vec<String>,
    active_line: String,
    replace_on_next_char: bool,
    pending_bytes: Vec<u8>,
}

impl TerminalBuffer {
    fn new() -> Self {
        Self {
            committed_lines: Vec::new(),
            active_line: String::new(),
            replace_on_next_char: false,
            pending_bytes: Vec::new(),
        }
    }

    fn feed(&mut self, data: &[u8]) {
        self.pending_bytes.extend_from_slice(data);

        let text = decode_cli_bytes_lossy(&self.pending_bytes);
        self.pending_bytes.clear();

        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                        self.commit_active_line();
                    } else {
                        self.replace_on_next_char = true;
                    }
                }
                '\n' => {
                    self.commit_active_line();
                }
                _ => {
                    if self.replace_on_next_char {
                        self.active_line.clear();
                        self.replace_on_next_char = false;
                    }
                    self.active_line.push(ch);
                }
            }
        }
    }

    fn commit_active_line(&mut self) {
        let line = self.active_line.trim().to_string();
        if !line.is_empty() {
            self.committed_lines.push(line);
        }
        self.active_line.clear();
        self.replace_on_next_char = false;
    }

    fn take_committed(&mut self) -> Vec<String> {
        std::mem::take(&mut self.committed_lines)
    }

    fn active_line(&self) -> String {
        self.active_line.clone()
    }

    fn finish(&mut self) {
        if !self.active_line.trim().is_empty() {
            self.commit_active_line();
        }
    }
}

async fn read_cli_stream<R>(
    mut stream: R,
    task_id: String,
    app_handle: tauri::AppHandle,
    log_prefix: Option<&'static str>,
)
where
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

                let active = term.active_line().trim().to_string();
                emit_active_line(&active, &task_id, &app_handle, log_prefix);
            }
            Err(_) => break,
        }
    }

    term.finish();
    for line in term.take_committed() {
        emit_committed_line(&line, &task_id, &app_handle, log_prefix);
    }
    emit_active_line("", &task_id, &app_handle, log_prefix);
}

#[cfg(test)]
fn take_cli_segment(input: &[u8]) -> Option<(String, Vec<u8>)> {
    let split_at = input.iter().position(|byte| *byte == b'\r' || *byte == b'\n')?;
    let mut rest_start = split_at + 1;
    if input.get(split_at) == Some(&b'\r') && input.get(split_at + 1) == Some(&b'\n') {
        rest_start = split_at + 2;
    }

    let segment = decode_cli_bytes_lossy(&input[..split_at]).trim().to_string();
    let rest = input[rest_start..].to_vec();
    Some((segment, rest))
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

fn decode_cli_bytes_lossy(bytes: &[u8]) -> String {
    if let Ok(decoded) = std::str::from_utf8(bytes) {
        return decoded.to_string();
    }

    cli_output_fallback_encoding().decode(bytes).0.into_owned()
}

fn cli_output_fallback_encoding() -> &'static Encoding {
    #[cfg(target_os = "windows")]
    {
        GB18030
    }

    #[cfg(not(target_os = "windows"))]
    {
        encoding_rs::UTF_8
    }
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

    let stderr = decode_cli_bytes_lossy(&output.stderr).to_lowercase();
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
        decode_cli_bytes_lossy, find_cli_in_ancestors, parse_progress, parse_speed,
        parse_threads, take_cli_segment, TaskRunner, TerminalBuffer,
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
    fn parse_progress_reads_real_cli_progress_reporter_line() {
        let line =
            "12:34:56.789 Progress: 10/40 (25.00%) -- 1.00MB/4.00MB (512.5KB/s @ 00:00:06)";
        assert_eq!(parse_progress(line), Some(0.25));
        assert_eq!(parse_speed(line).as_deref(), Some("512.5KB/s"));
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
        let (segment, rest) = take_cli_segment(b"Progress: 1/2 (50.00%)\rnext").expect("segment");
        assert_eq!(segment, "Progress: 1/2 (50.00%)");
        assert_eq!(rest, b"next".to_vec());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn decode_cli_bytes_lossy_prefers_utf8_when_valid() {
        let bytes = "文件名称：测试".as_bytes();
        assert_eq!(decode_cli_bytes_lossy(bytes), "文件名称：测试");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn decode_cli_bytes_lossy_falls_back_for_non_utf8_bytes() {
        let (encoded, _, _) = encoding_rs::GB18030.encode("文件名称：测试");
        assert_eq!(decode_cli_bytes_lossy(&encoded), "文件名称：测试");
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

    #[test]
    fn terminal_buffer_cr_overwrites_active_line() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"Progress: 1/10\rProgress: 2/10\rProgress: 3/10");
        assert_eq!(buf.active_line(), "Progress: 3/10");
        assert!(buf.take_committed().is_empty());
    }

    #[test]
    fn terminal_buffer_lf_commits_to_history() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"line one\nline two\n");
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["line one", "line two"]);
        assert_eq!(buf.active_line(), "");
    }

    #[test]
    fn terminal_buffer_mixed_cr_and_lf() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"Starting download\nProgress: 1/10\rProgress: 2/10\rProgress: 3/10\nDone\n");
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["Starting download", "Progress: 3/10", "Done"]);
        assert_eq!(buf.active_line(), "");
    }

    #[test]
    fn terminal_buffer_keeps_progress_reporter_line_active_after_trailing_cr() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"\rProgress: 1/10 (10.00%) -- 1MB/10MB\r");
        assert_eq!(buf.active_line(), "Progress: 1/10 (10.00%) -- 1MB/10MB");
        assert!(buf.take_committed().is_empty());
    }

    #[test]
    fn terminal_buffer_overwrites_progress_reporter_line_without_committing_history() {
        let mut buf = TerminalBuffer::new();
        buf.feed(
            b"\rProgress: 1/10 (10.00%) -- 1MB/10MB\r\rProgress: 2/10 (20.00%) -- 2MB/10MB\r",
        );
        assert_eq!(buf.active_line(), "Progress: 2/10 (20.00%) -- 2MB/10MB");
        assert!(buf.take_committed().is_empty());
    }

    #[test]
    fn terminal_buffer_replaces_longer_progress_tail_with_shorter_status_line() {
        let mut buf = TerminalBuffer::new();
        buf.feed(
            b"\rProgress: 1650/1657 (99.58%) -- 1.14 GB/1.15 GB (793.51 KB/s @ 00m06s18s))\r\r11:07:37.504 \xe5\xb7\xb2\xe4\xb8\x8b\xe8\xbd\xbd\xe5\xae\x8c\xe6\x88\x90\r",
        );
        assert_eq!(buf.active_line(), "11:07:37.504 已下载完成");
        assert!(buf.take_committed().is_empty());
    }

    #[test]
    fn terminal_buffer_does_not_commit_progress_line_when_logger_prints_next_line() {
        let mut buf = TerminalBuffer::new();
        buf.feed(
            b"\rProgress: 3/10 (30.00%) -- 3MB/10MB\r\r                                        \r11:00:00.000 \xe7\xad\x89\xe5\xbe\x85\xe4\xb8\x8b\xe8\xbd\xbd\xe5\xae\x8c\xe6\x88\x90...\n",
        );
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["11:00:00.000 等待下载完成..."]);
        assert_eq!(buf.active_line(), "");
    }

    #[test]
    fn terminal_buffer_crlf_treated_as_newline() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"hello\r\nworld\r\n");
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["hello", "world"]);
    }

    #[test]
    fn terminal_buffer_finish_commits_trailing_active_line() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"trailing text");
        assert_eq!(buf.active_line(), "trailing text");
        buf.finish();
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["trailing text"]);
    }
}
