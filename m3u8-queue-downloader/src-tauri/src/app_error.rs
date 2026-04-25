use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("Task {id} not found")]
    TaskNotFound { id: String },
    #[error("Cannot {action} task {id} with status {status}")]
    InvalidTaskStatus {
        action: &'static str,
        id: String,
        status: String,
    },
    #[error("{name} not found in bundled resources or any searched directory")]
    CliExecutableNotFound { name: String },
    #[error("Only completed and failed tasks can be stored in history")]
    InvalidHistoryStatus,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        Self::Message(value.to_string())
    }
}
