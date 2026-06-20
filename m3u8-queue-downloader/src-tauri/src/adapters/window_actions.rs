use crate::application::app_error::{AppError, AppResult};
use tauri::{AppHandle, Manager};

pub(crate) fn show_main_window(app_handle: &AppHandle) -> AppResult<()> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .show()
            .map_err(|e| AppError::message(e.to_string()))?;
        window
            .unminimize()
            .map_err(|e| AppError::message(e.to_string()))?;
        window
            .set_focus()
            .map_err(|e| AppError::message(e.to_string()))?;
    }
    Ok(())
}

pub(crate) fn hide_main_window(app_handle: &AppHandle) -> AppResult<()> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .hide()
            .map_err(|e| AppError::message(e.to_string()))?;
    }
    Ok(())
}

pub(crate) fn minimize_main_window(app_handle: &AppHandle) -> AppResult<()> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .minimize()
            .map_err(|e| AppError::message(e.to_string()))?;
    }
    Ok(())
}

pub(crate) fn toggle_main_window_maximize(app_handle: &AppHandle) -> AppResult<()> {
    if let Some(window) = app_handle.get_webview_window("main") {
        if window
            .is_maximized()
            .map_err(|e| AppError::message(e.to_string()))?
        {
            window
                .unmaximize()
                .map_err(|e| AppError::message(e.to_string()))?;
        } else {
            window
                .maximize()
                .map_err(|e| AppError::message(e.to_string()))?;
        }
    }
    Ok(())
}
