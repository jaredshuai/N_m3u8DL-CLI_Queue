use crate::app_error::AppResult;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::Mutex;

const SHUTDOWN_SECONDS: u64 = 60;

#[derive(Debug, Default)]
struct ShutdownState {
    run_had_failure: bool,
    countdown_pending: bool,
    cancelled_until_reenabled: bool,
}

pub struct ShutdownManager {
    state: Mutex<ShutdownState>,
}

impl ShutdownManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ShutdownState::default()),
        }
    }

    pub fn reset_for_new_run(&self) -> AppResult<bool> {
        let countdown_pending = {
            let state = self.state.lock().expect("shutdown mutex poisoned");
            state.countdown_pending
        };

        if countdown_pending && !cfg!(test) {
            cancel_shutdown()?;
        }

        let mut state = self.state.lock().expect("shutdown mutex poisoned");
        state.run_had_failure = false;
        state.countdown_pending = false;
        state.cancelled_until_reenabled = false;
        Ok(countdown_pending)
    }

    pub fn mark_run_failure(&self) {
        let mut state = self.state.lock().expect("shutdown mutex poisoned");
        state.run_had_failure = true;
    }

    pub fn clear_cancellation_after_reenable(&self) {
        let mut state = self.state.lock().expect("shutdown mutex poisoned");
        state.cancelled_until_reenabled = false;
    }

    pub fn should_start_countdown(&self) -> bool {
        let state = self.state.lock().expect("shutdown mutex poisoned");
        !state.run_had_failure && !state.countdown_pending && !state.cancelled_until_reenabled
    }

    pub fn start_countdown(&self) -> AppResult<u64> {
        if !cfg!(test) {
            schedule_shutdown(SHUTDOWN_SECONDS)?;
        }
        let mut state = self.state.lock().expect("shutdown mutex poisoned");
        state.countdown_pending = true;
        Ok(SHUTDOWN_SECONDS)
    }

    pub fn cancel_countdown(&self) -> AppResult<()> {
        if !cfg!(test) {
            cancel_shutdown()?;
        }
        let mut state = self.state.lock().expect("shutdown mutex poisoned");
        state.countdown_pending = false;
        state.cancelled_until_reenabled = true;
        Ok(())
    }
}

impl Default for ShutdownManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub fn shutdown_seconds() -> u64 {
    SHUTDOWN_SECONDS
}

#[cfg(target_os = "windows")]
fn schedule_shutdown(seconds: u64) -> AppResult<()> {
    let status = Command::new("shutdown")
        .args(["/s", "/t", &seconds.to_string()])
        .creation_flags(0x08000000)
        .status()
        .map_err(|e| {
            crate::app_error::AppError::message(format!("failed to schedule Windows shutdown: {e}"))
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "shutdown /s /t {seconds} failed with exit code {}",
            status.code().unwrap_or(-1)
        )
        .into())
    }
}

#[cfg(not(target_os = "windows"))]
fn schedule_shutdown(_seconds: u64) -> AppResult<()> {
    let _ = Command::new("true");
    Err("automatic shutdown is only supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
fn cancel_shutdown() -> AppResult<()> {
    let status = Command::new("shutdown")
        .args(["/a"])
        .creation_flags(0x08000000)
        .status()
        .map_err(|e| {
            crate::app_error::AppError::message(format!("failed to cancel Windows shutdown: {e}"))
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "shutdown /a failed with exit code {}",
            status.code().unwrap_or(-1)
        )
        .into())
    }
}

#[cfg(not(target_os = "windows"))]
fn cancel_shutdown() -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ShutdownManager;

    #[test]
    fn failed_run_blocks_countdown_until_reset() {
        let manager = ShutdownManager::new();
        manager.mark_run_failure();
        assert!(!manager.should_start_countdown());

        manager.reset_for_new_run().expect("reset for new run");
        assert!(manager.should_start_countdown());
    }

    #[test]
    fn cancel_blocks_restart_until_reenabled() {
        let manager = ShutdownManager::new();
        manager.cancel_countdown().expect("cancel countdown");
        assert!(!manager.should_start_countdown());

        manager.clear_cancellation_after_reenable();
        assert!(manager.should_start_countdown());
    }

    #[test]
    fn reset_for_new_run_clears_pending_and_reenables_shutdown_logic() {
        let manager = ShutdownManager::new();
        manager.start_countdown().expect("start countdown");
        assert!(!manager.should_start_countdown());

        let cancelled = manager.reset_for_new_run().expect("reset for new run");
        assert!(cancelled);
        assert!(manager.should_start_countdown());
    }
}
