use crate::adapters::window_actions;
use crate::application::app_error::AppResult;
use crate::ports::application_control::ApplicationControl;
use tauri::AppHandle;

pub(crate) struct TauriApplicationControl {
    app_handle: AppHandle,
}

impl TauriApplicationControl {
    pub(crate) fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl ApplicationControl for TauriApplicationControl {
    fn hide_main_window(&self) -> AppResult<()> {
        window_actions::hide_main_window(&self.app_handle)
    }

    fn exit(&self, code: i32) {
        self.app_handle.exit(code);
    }
}
