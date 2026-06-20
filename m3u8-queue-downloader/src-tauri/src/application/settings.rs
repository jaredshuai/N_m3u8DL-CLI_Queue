#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CloseButtonBehavior {
    #[default]
    CloseToTray,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettings {
    pub close_button_behavior: CloseButtonBehavior,
    pub auto_action_on_complete: bool,
    pub download_dir: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_button_behavior: CloseButtonBehavior::CloseToTray,
            auto_action_on_complete: false,
            download_dir: None,
        }
    }
}

pub fn normalize_download_dir(input: Option<String>) -> Option<String> {
    input
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_safe_for_desktop_startup() {
        let settings = AppSettings::default();

        assert_eq!(
            settings.close_button_behavior,
            CloseButtonBehavior::CloseToTray
        );
        assert!(!settings.auto_action_on_complete);
        assert!(settings.download_dir.is_none());
    }

    #[test]
    fn normalize_download_dir_trims_and_drops_empty_values() {
        assert_eq!(
            normalize_download_dir(Some("  D:/Videos  ".to_string())),
            Some("D:/Videos".to_string())
        );
        assert_eq!(normalize_download_dir(Some("   ".to_string())), None);
        assert_eq!(normalize_download_dir(None), None);
    }
}
