use crate::models::AppSettings;
use std::env;
use std::path::PathBuf;

pub fn normalize_download_dir(input: Option<String>) -> Option<String> {
    input
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn resolve_download_dir(settings: &AppSettings) -> PathBuf {
    if let Some(custom) = normalize_download_dir(settings.download_dir.clone()) {
        return PathBuf::from(custom);
    }

    detect_default_download_dir()
}

fn detect_default_download_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(path) = detect_windows_video_dir_from_registry() {
        return path;
    }

    dirs::video_dir()
        .or_else(dirs::download_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(target_os = "windows")]
fn detect_windows_video_dir_from_registry() -> Option<PathBuf> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders")
        .ok()?;

    let value = key.get_value::<String, _>("My Video").ok().or_else(|| {
        key.get_value::<String, _>("{35286A68-3C57-41A1-BBB1-0EAE73d76C95}")
            .ok()
    })?;

    Some(PathBuf::from(expand_windows_env_vars(&value)))
}

#[cfg(target_os = "windows")]
fn expand_windows_env_vars(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }

        let mut var_name = String::new();
        while let Some(&next) = chars.peek() {
            chars.next();
            if next == '%' {
                break;
            }
            var_name.push(next);
        }

        if var_name.is_empty() {
            result.push('%');
            continue;
        }

        match env::var(&var_name) {
            Ok(value) => result.push_str(&value),
            Err(_) => {
                result.push('%');
                result.push_str(&var_name);
                result.push('%');
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_download_dir_trims_and_drops_empty_values() {
        assert_eq!(
            normalize_download_dir(Some("  D:/Videos  ".to_string())),
            Some("D:/Videos".to_string())
        );
        assert_eq!(normalize_download_dir(Some("   ".to_string())), None);
        assert_eq!(normalize_download_dir(None), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn expand_windows_env_vars_expands_known_values() {
        let expanded = expand_windows_env_vars("%USERPROFILE%\\Videos");
        assert!(expanded.contains("Videos"));
        assert_ne!(expanded, "%USERPROFILE%\\Videos");
    }
}
