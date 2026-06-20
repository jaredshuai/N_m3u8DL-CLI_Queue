use crate::application::app_error::AppResult;
use crate::application::download_directory::DownloadDirectory;
use std::sync::Arc;

pub(crate) trait DirectoryOpener: Send + Sync {
    fn open_directory(&self, directory: &DownloadDirectory) -> AppResult<()>;
}

impl<T> DirectoryOpener for Arc<T>
where
    T: DirectoryOpener + ?Sized,
{
    fn open_directory(&self, directory: &DownloadDirectory) -> AppResult<()> {
        self.as_ref().open_directory(directory)
    }
}
