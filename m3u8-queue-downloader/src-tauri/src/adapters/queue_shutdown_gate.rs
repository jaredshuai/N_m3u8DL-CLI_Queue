use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) struct QueueShutdownGate {
    shutting_down: Arc<Mutex<bool>>,
}

impl QueueShutdownGate {
    pub(crate) fn new() -> Self {
        Self {
            shutting_down: Arc::new(Mutex::new(false)),
        }
    }

    pub(crate) async fn mark_shutting_down(&self) {
        let mut shutting_down = self.shutting_down.lock().await;
        *shutting_down = true;
    }

    pub(crate) async fn clear_shutdown_flag(&self) {
        let mut shutting_down = self.shutting_down.lock().await;
        *shutting_down = false;
    }

    pub(crate) async fn status(&self) -> bool {
        *self.shutting_down.lock().await
    }
}
