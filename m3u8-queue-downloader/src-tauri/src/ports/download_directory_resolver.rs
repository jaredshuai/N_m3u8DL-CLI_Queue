use crate::application::download_directory::DownloadDirectory;
use crate::application::settings::AppSettings;

pub(crate) trait DownloadDirectoryResolver: Send + Sync {
    fn resolve_download_dir(&self, settings: &AppSettings) -> DownloadDirectory;
}
