use crate::application::app_error::AppError;

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::message(error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::message(error.to_string())
    }
}
