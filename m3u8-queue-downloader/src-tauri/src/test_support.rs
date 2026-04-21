#![cfg(test)]

use tokio::process::{Child, Command};

pub async fn spawn_sleeping_child() -> Child {
    Command::new("powershell")
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
        .spawn()
        .expect("spawn sleeping child")
}
