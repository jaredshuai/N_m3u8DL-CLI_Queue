use crate::application::app_error::AppResult;
use crate::application::DirectoryOpener;
use crate::application::DownloadDirectoryResolver;
use crate::application::SettingsRepository;

pub(crate) struct DownloadDirectoryPorts<'a> {
    settings_repository: &'a dyn SettingsRepository,
    download_directory_resolver: &'a dyn DownloadDirectoryResolver,
    directory_opener: &'a dyn DirectoryOpener,
}

impl<'a> DownloadDirectoryPorts<'a> {
    pub(crate) fn new(
        settings_repository: &'a dyn SettingsRepository,
        download_directory_resolver: &'a dyn DownloadDirectoryResolver,
        directory_opener: &'a dyn DirectoryOpener,
    ) -> Self {
        Self {
            settings_repository,
            download_directory_resolver,
            directory_opener,
        }
    }

    pub(crate) fn open_download_dir(&self) -> AppResult<()> {
        let download_dir = self
            .download_directory_resolver
            .resolve_download_dir(&self.settings_repository.get());
        self.directory_opener.open_directory(&download_dir)
    }
}
