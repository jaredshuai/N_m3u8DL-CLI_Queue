use crate::application::download_directory::DownloadDirectory;

#[derive(Debug, Clone)]
pub(crate) struct TaskProcessStartRequest {
    pub task_id: String,
    pub url: String,
    pub save_name: Option<String>,
    pub headers: Option<String>,
    pub download_dir: DownloadDirectory,
}
