use crate::application::app_error::{AppError, AppResult};
use crate::domain::history::HistoryStatus;

pub(crate) fn parse_history_status(status: &str) -> AppResult<HistoryStatus> {
    match status {
        "completed" => Ok(HistoryStatus::Completed),
        "failed" => Ok(HistoryStatus::Failed),
        _ => Err(AppError::InvalidHistoryStatus),
    }
}

pub(crate) fn history_status_slug(status: HistoryStatus) -> &'static str {
    match status {
        HistoryStatus::Completed => "completed",
        HistoryStatus::Failed => "failed",
    }
}
