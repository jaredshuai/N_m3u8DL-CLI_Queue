use crate::application::settings::{AppSettings, CloseButtonBehavior, ThemePreference};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum CloseButtonBehaviorDto {
    #[default]
    CloseToTray,
    Exit,
}

impl From<&CloseButtonBehavior> for CloseButtonBehaviorDto {
    fn from(value: &CloseButtonBehavior) -> Self {
        match value {
            CloseButtonBehavior::CloseToTray => Self::CloseToTray,
            CloseButtonBehavior::Exit => Self::Exit,
        }
    }
}

impl From<CloseButtonBehaviorDto> for CloseButtonBehavior {
    fn from(value: CloseButtonBehaviorDto) -> Self {
        match value {
            CloseButtonBehaviorDto::CloseToTray => Self::CloseToTray,
            CloseButtonBehaviorDto::Exit => Self::Exit,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum ThemePreferenceDto {
    #[default]
    Auto,
    Dark,
    Light,
}

impl From<&ThemePreference> for ThemePreferenceDto {
    fn from(value: &ThemePreference) -> Self {
        match value {
            ThemePreference::Auto => Self::Auto,
            ThemePreference::Dark => Self::Dark,
            ThemePreference::Light => Self::Light,
        }
    }
}

impl From<ThemePreferenceDto> for ThemePreference {
    fn from(value: ThemePreferenceDto) -> Self {
        match value {
            ThemePreferenceDto::Auto => Self::Auto,
            ThemePreferenceDto::Dark => Self::Dark,
            ThemePreferenceDto::Light => Self::Light,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    #[serde(default)]
    pub close_button_behavior: CloseButtonBehaviorDto,
    #[serde(default, rename = "autoShutdownOnComplete")]
    pub auto_action_on_complete: bool,
    #[serde(default, rename = "downloadDir")]
    pub download_dir: Option<String>,
    #[serde(default)]
    pub theme: ThemePreferenceDto,
}

impl From<&AppSettings> for AppSettingsDto {
    fn from(settings: &AppSettings) -> Self {
        Self {
            close_button_behavior: CloseButtonBehaviorDto::from(&settings.close_button_behavior),
            auto_action_on_complete: settings.auto_action_on_complete,
            download_dir: settings.download_dir.clone(),
            theme: ThemePreferenceDto::from(&settings.theme),
        }
    }
}

impl From<AppSettings> for AppSettingsDto {
    fn from(settings: AppSettings) -> Self {
        Self::from(&settings)
    }
}

impl From<AppSettingsDto> for AppSettings {
    fn from(settings: AppSettingsDto) -> Self {
        Self {
            close_button_behavior: settings.close_button_behavior.into(),
            auto_action_on_complete: settings.auto_action_on_complete,
            download_dir: settings.download_dir,
            theme: settings.theme.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_dto_preserves_frontend_json_shape() {
        let settings = AppSettings {
            close_button_behavior: CloseButtonBehavior::Exit,
            auto_action_on_complete: true,
            download_dir: Some("D:/Videos".to_string()),
            theme: ThemePreference::Light,
        };

        let value =
            serde_json::to_value(AppSettingsDto::from(&settings)).expect("serialize settings dto");

        assert_eq!(value["closeButtonBehavior"], serde_json::json!("exit"));
        assert_eq!(value["autoShutdownOnComplete"], serde_json::json!(true));
        assert_eq!(value["downloadDir"], serde_json::json!("D:/Videos"));
        assert_eq!(value["theme"], serde_json::json!("light"));
    }

    #[test]
    fn settings_dto_defaults_missing_fields() {
        let dto: AppSettingsDto = serde_json::from_str("{}").expect("deserialize defaults");
        let settings = AppSettings::from(dto);

        assert_eq!(
            settings.close_button_behavior,
            CloseButtonBehavior::CloseToTray
        );
        assert!(!settings.auto_action_on_complete);
        assert!(settings.download_dir.is_none());
        assert_eq!(settings.theme, ThemePreference::Auto);
    }
}
