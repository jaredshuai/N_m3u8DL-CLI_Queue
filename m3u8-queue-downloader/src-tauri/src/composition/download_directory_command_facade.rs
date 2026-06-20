use crate::application::app_error::AppResult;
use crate::composition::dependency_graph::DependencyGraph;

pub(crate) struct DownloadDirectoryCommandFacade {
    dependencies: DependencyGraph,
}

impl DownloadDirectoryCommandFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    pub(crate) fn open_download_dir(&self) -> AppResult<()> {
        let ports = self.dependencies.download_directory_orchestrator();
        ports.open_download_dir()
    }
}
