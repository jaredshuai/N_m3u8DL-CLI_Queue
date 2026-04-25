use crate::app_state::AppState;
use crate::download_dir::resolve_download_dir;
use crate::runtime::{self, CloseRequestSource};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMenuAction {
    ShowMainWindow,
    StartQueue,
    PauseQueue,
    OpenDownloadDir,
    RequestClose(CloseRequestSource),
    Ignore,
}

pub(crate) fn request_close_from_handle(app_handle: tauri::AppHandle, source: CloseRequestSource) {
    let state = app_handle.state::<AppState>().inner().clone();

    tauri::async_runtime::spawn(async move {
        if let Err(err) = runtime::request_close_internal(&state, app_handle, source).await {
            eprintln!("Failed to process close request: {}", err);
        }
    });
}

fn tray_menu_action(id: &str) -> TrayMenuAction {
    match id {
        "show-main-window" => TrayMenuAction::ShowMainWindow,
        "start-queue" => TrayMenuAction::StartQueue,
        "pause-queue" => TrayMenuAction::PauseQueue,
        "open-download-dir" => TrayMenuAction::OpenDownloadDir,
        "quit" => TrayMenuAction::RequestClose(CloseRequestSource::TrayQuit),
        _ => TrayMenuAction::Ignore,
    }
}

pub(crate) fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show-main-window", "显示主窗口", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "start-queue", "开始/恢复队列", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause-queue", "暂停后续任务", true, None::<&str>)?;
    let open_dir = MenuItem::with_id(app, "open-download-dir", "打开下载目录", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &start, &pause, &open_dir, &quit])?;

    let icon = app.default_window_icon().cloned();
    let mut tray_builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("m3u8 队列下载器")
        .on_menu_event(
            move |app, event| match tray_menu_action(event.id.as_ref()) {
                TrayMenuAction::ShowMainWindow => {
                    let _ = runtime::show_main_window(app);
                }
                TrayMenuAction::StartQueue => start_queue_from_handle(app.clone()),
                TrayMenuAction::PauseQueue => pause_queue_from_handle(app.clone()),
                TrayMenuAction::OpenDownloadDir => {
                    let state = app.state::<AppState>();
                    let path = resolve_download_dir(&state.settings_store.get());
                    if let Err(err) = runtime::open_download_dir_for_path(path) {
                        eprintln!("Failed to open download directory from tray: {}", err);
                    }
                }
                TrayMenuAction::RequestClose(source) => {
                    request_close_from_handle(app.clone(), source)
                }
                TrayMenuAction::Ignore => {}
            },
        )
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = runtime::show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = icon {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_quit_menu_is_wired_as_tray_quit_source() {
        assert_eq!(
            tray_menu_action("quit"),
            TrayMenuAction::RequestClose(CloseRequestSource::TrayQuit)
        );
    }
}

fn start_queue_from_handle(app_handle: tauri::AppHandle) {
    let state = app_handle.state::<AppState>().inner().clone();

    tauri::async_runtime::spawn(async move {
        if let Err(err) = runtime::start_queue_internal(&state, &app_handle).await {
            eprintln!("Failed to start queue from tray: {}", err);
        }
    });
}

fn pause_queue_from_handle(app_handle: tauri::AppHandle) {
    let state = app_handle.state::<AppState>();
    let queue_manager = state.queue_manager.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(err) = runtime::pause_queue_internal(&queue_manager).await {
            eprintln!("Failed to pause queue from tray: {}", err);
            return;
        }
        let _ = app_handle.emit("queue-state-changed", ());
    });
}
