use crate::application::app_error::{AppError, AppResult};
use crate::application::download_directory::DownloadDirectory;
use crate::ports::directory_opener::DirectoryOpener;
use std::path::Path;

pub(crate) struct SystemDirectoryOpener;

impl DirectoryOpener for SystemDirectoryOpener {
    fn open_directory(&self, directory: &DownloadDirectory) -> AppResult<()> {
        let path = Path::new(directory.as_str());
        std::fs::create_dir_all(path)?;

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()
                .map_err(|e| AppError::message(format!("Failed to open directory: {e}")))?;
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(path)
                .spawn()
                .map_err(|e| AppError::message(format!("Failed to open directory: {e}")))?;
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| AppError::message(format!("Failed to open directory: {e}")))?;
        }

        Ok(())
    }
}
