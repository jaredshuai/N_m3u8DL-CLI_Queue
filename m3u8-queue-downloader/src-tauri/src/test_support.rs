#![cfg(test)]

use tokio::process::{Child, Command};

#[cfg(target_os = "windows")]
pub async fn spawn_sleeping_child() -> Child {
    Command::new("powershell")
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 2"])
        .spawn()
        .expect("spawn sleeping child")
}

#[cfg(target_os = "windows")]
pub async fn spawn_success_child() -> Child {
    Command::new("powershell")
        .args(["-NoProfile", "-Command", "exit 0"])
        .spawn()
        .expect("spawn success child")
}

#[cfg(not(target_os = "windows"))]
pub async fn spawn_sleeping_child() -> Child {
    Command::new("sh")
        .args(["-c", "sleep 2"])
        .spawn()
        .expect("spawn sleeping child")
}

#[cfg(not(target_os = "windows"))]
pub async fn spawn_success_child() -> Child {
    Command::new("sh")
        .args(["-c", "exit 0"])
        .spawn()
        .expect("spawn success child")
}
