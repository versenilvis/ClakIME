use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutostartStatus {
    pub is_enabled: bool,
    pub user_autostart_desktop_exists: bool,
    pub system_fcitx5_autostart_exists: bool,
    pub env_conf_exists: bool,
    pub profile_contains_clak: bool,
}

// read-only autostart inspection without modifying any files
pub fn check_autostart() -> AutostartStatus {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let autostart_file = PathBuf::from(&home).join(".config/autostart/clak-autostart.desktop");
    let user_autostart_desktop_exists = autostart_file.exists();

    let fcitx_autostart = PathBuf::from("/etc/xdg/autostart/org.fcitx.Fcitx5.desktop");
    let system_fcitx5_autostart_exists = fcitx_autostart.exists();

    let env_file = PathBuf::from(&home).join(".config/environment.d/99-clak-im.conf");
    let env_conf_exists = env_file.exists();

    let profile_file = PathBuf::from(&home).join(".config/fcitx5/profile");
    let profile_contains_clak = if profile_file.exists() {
        fs::read_to_string(&profile_file)
            .map(|s| s.contains("Name=clak"))
            .unwrap_or(false)
    } else {
        false
    };

    let is_enabled = user_autostart_desktop_exists || system_fcitx5_autostart_exists;

    AutostartStatus {
        is_enabled,
        user_autostart_desktop_exists,
        system_fcitx5_autostart_exists,
        env_conf_exists,
        profile_contains_clak,
    }
}
