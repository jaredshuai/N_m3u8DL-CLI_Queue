#![cfg(test)]

use tokio::process::{Child, Command};

#[cfg(target_os = "windows")]
pub async fn spawn_sleeping_child() -> Child {
    Command::new("powershell")
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
        .spawn()
        .expect("spawn sleeping child")
}

#[cfg(not(target_os = "windows"))]
pub async fn spawn_sleeping_child() -> Child {
    Command::new("sh")
        .args(["-c", "sleep 30"])
        .spawn()
        .expect("spawn sleeping child")
}
