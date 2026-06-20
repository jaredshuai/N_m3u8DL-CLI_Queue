use crate::application::app_error::AppResult;
use std::sync::Arc;

pub(crate) trait ApplicationControl: Send + Sync {
    fn hide_main_window(&self) -> AppResult<()>;
    fn exit(&self, code: i32);
}

impl<T> ApplicationControl for Arc<T>
where
    T: ApplicationControl + ?Sized,
{
    fn hide_main_window(&self) -> AppResult<()> {
        self.as_ref().hide_main_window()
    }

    fn exit(&self, code: i32) {
        self.as_ref().exit(code);
    }
}
