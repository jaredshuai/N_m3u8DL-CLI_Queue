use crate::application::close_policy::CloseRequestSource;
use crate::composition::tray;
use tauri::{Manager, WindowEvent};

pub(crate) fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        if handle_main_window_close_requested(window.label(), |source| {
            tray::request_close_from_handle(window.app_handle().clone(), source);
        }) {
            api.prevent_close();
        }
    }
}

fn handle_main_window_close_requested<F>(window_label: &str, mut request_close: F) -> bool
where
    F: FnMut(CloseRequestSource),
{
    if window_label != "main" {
        return false;
    }

    request_close(CloseRequestSource::WindowButton);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_window_close_is_wired_as_window_button_source() {
        let mut requested_source = None;

        let handled = handle_main_window_close_requested("main", |source| {
            requested_source = Some(source);
        });

        assert!(handled);
        assert_eq!(requested_source, Some(CloseRequestSource::WindowButton));
    }

    #[test]
    fn non_main_window_close_is_not_handled_by_main_close_wiring() {
        let mut requested_source = None;

        let handled = handle_main_window_close_requested("secondary", |source| {
            requested_source = Some(source);
        });

        assert!(!handled);
        assert_eq!(requested_source, None);
    }
}
