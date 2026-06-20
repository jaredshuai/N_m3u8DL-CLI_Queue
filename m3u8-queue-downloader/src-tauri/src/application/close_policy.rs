use crate::application::settings::CloseButtonBehavior;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseRequestSource {
    WindowButton,
    TrayQuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseAction {
    HideToTray,
    ExitApplication,
}

pub(crate) fn resolve_close_action(
    behavior: CloseButtonBehavior,
    source: CloseRequestSource,
) -> CloseAction {
    match source {
        CloseRequestSource::TrayQuit => CloseAction::ExitApplication,
        CloseRequestSource::WindowButton => match behavior {
            CloseButtonBehavior::CloseToTray => CloseAction::HideToTray,
            CloseButtonBehavior::Exit => CloseAction::ExitApplication,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_quit_always_exits_even_when_window_close_hides_to_tray() {
        assert_eq!(
            resolve_close_action(
                CloseButtonBehavior::CloseToTray,
                CloseRequestSource::TrayQuit,
            ),
            CloseAction::ExitApplication
        );
    }

    #[test]
    fn window_close_respects_close_to_tray_setting() {
        assert_eq!(
            resolve_close_action(
                CloseButtonBehavior::CloseToTray,
                CloseRequestSource::WindowButton,
            ),
            CloseAction::HideToTray
        );
        assert_eq!(
            resolve_close_action(CloseButtonBehavior::Exit, CloseRequestSource::WindowButton),
            CloseAction::ExitApplication
        );
    }
}
