use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClakConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub spelling: SpellingConfig,
    #[serde(default)]
    pub typing: TypingConfig,
    #[serde(default)]
    pub shortcuts: ShortcutConfig,
    #[serde(default)]
    pub per_app: PerAppConfig,
    #[serde(default)]
    pub macros: MacroConfig,
    #[serde(default)]
    pub advanced: AdvancedConfig,
    #[serde(default)]
    pub update: UpdateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    #[serde(default = "default_true")]
    pub auto_update: bool,
    #[serde(default = "default_poll_index")]
    pub poll_index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_charset")]
    pub charset: String,
    #[serde(default = "default_true")]
    pub short_w: bool,
    #[serde(default = "default_true")]
    pub bracket_brackets: bool,
    #[serde(default = "default_startup_mode")]
    pub startup_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_restore: bool,
    #[serde(default = "default_true")]
    pub modern_tone: bool,
    #[serde(default = "default_true")]
    pub free_marking: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypingConfig {
    #[serde(default)]
    pub double_space_period: bool,
    #[serde(default)]
    pub auto_capitalize: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    #[serde(default = "default_toggle_shortcut")]
    pub toggle_vietnamese: String,
    #[serde(default = "default_switch_shortcut")]
    pub switch_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerAppConfig {
    #[serde(default = "default_true")]
    pub remember_state: bool,
    #[serde(default = "default_excluded_apps")]
    pub excluded_apps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_macro_items")]
    pub items: Vec<MacroItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroItem {
    pub trigger: String,
    pub replace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    #[serde(default = "default_true")]
    pub autostart: bool,
    #[serde(default)]
    pub uinput_ack: bool,
    #[serde(default)]
    pub debug_log: bool,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            autostart: default_true(),
            uinput_ack: false,
            debug_log: false,
        }
    }
}

fn default_method() -> String {
    "telex".to_string()
}

fn default_charset() -> String {
    "unicode".to_string()
}

fn default_true() -> bool {
    true
}

fn default_startup_mode() -> String {
    "vietnamese".to_string()
}

fn default_toggle_shortcut() -> String {
    "ctrl_shift".to_string()
}

fn default_switch_shortcut() -> String {
    "ctrl_space".to_string()
}

fn default_excluded_apps() -> Vec<String> {
    Vec::new()
}

fn default_macro_items() -> Vec<MacroItem> {
    Vec::new()
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            method: default_method(),
            charset: default_charset(),
            short_w: default_true(),
            bracket_brackets: default_true(),
            startup_mode: default_startup_mode(),
        }
    }
}

impl Default for SpellingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_restore: false,
            modern_tone: default_true(),
            free_marking: default_true(),
        }
    }
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            toggle_vietnamese: default_toggle_shortcut(),
            switch_method: default_switch_shortcut(),
        }
    }
}

impl Default for PerAppConfig {
    fn default() -> Self {
        Self {
            remember_state: default_true(),
            excluded_apps: default_excluded_apps(),
        }
    }
}

impl Default for MacroConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            items: default_macro_items(),
        }
    }
}

fn default_poll_index() -> i32 {
    4
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_update: default_true(),
            poll_index: default_poll_index(),
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("clak").join("config.toml");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("clak")
            .join("config.toml");
    }
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir().join(format!("clak_{}_config.toml", uid))
}

impl ClakConfig {
    pub fn load() -> Self {
        let path = config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<ClakConfig>(&content) {
                    return cfg;
                }
            }
        }
        let default_cfg = ClakConfig::default();
        let _ = default_cfg.save();
        default_cfg
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(path, content)?;
        let _ = apply_autostart(self.advanced.autostart, &self.general.startup_mode);
        Ok(())
    }
}

// configure autostart entry and environment variables
pub fn apply_autostart(enabled: bool, startup_mode: &str) -> std::io::Result<()> {
    let home = match std::env::var("HOME") {
        Ok(h) if !h.is_empty() => PathBuf::from(h),
        _ => return Ok(()),
    };

    let autostart_dir = home.join(".config").join("autostart");
    let desktop_file = autostart_dir.join("clak-autostart.desktop");

    if enabled {
        let _ = fs::create_dir_all(&autostart_dir);
        let content = "[Desktop Entry]\nType=Application\nName=Clak Vietnamese Input Method\nComment=Autostart Fcitx5 with Clak input method\nExec=fcitx5 -d\nIcon=org.fcitx.Fcitx5\nTerminal=false\nCategories=System;Utility;\nStartupNotify=false\nX-GNOME-Autostart-Phase=Applications\nX-GNOME-AutoRestart=true\nX-GNOME-Autostart-Notify=false\nX-KDE-autostart-after=panel\n";
        let _ = fs::write(&desktop_file, content);

        let env_dir = home.join(".config").join("environment.d");
        let _ = fs::create_dir_all(&env_dir);
        let env_file = env_dir.join("99-clak-im.conf");
        let env_content =
            "QT_IM_MODULE=fcitx\nXMODIFIERS=@im=fcitx\nINPUT_METHOD=fcitx5\nSDL_IM_MODULE=fcitx\n";
        let _ = fs::write(&env_file, env_content);

        configure_fcitx5_profile(&home, startup_mode);
    } else {
        if desktop_file.exists() {
            let _ = fs::remove_file(&desktop_file);
        }
        let env_file = home
            .join(".config")
            .join("environment.d")
            .join("99-clak-im.conf");
        if env_file.exists() {
            let _ = fs::remove_file(&env_file);
        }
    }
    Ok(())
}

fn configure_fcitx5_profile(home: &std::path::Path, startup_mode: &str) {
    let profile_path = home.join(".config").join("fcitx5").join("profile");
    if let Some(parent) = profile_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let default_im = if startup_mode == "english" {
        "keyboard-us"
    } else {
        "clak"
    };

    if !profile_path.exists() {
        let content = format!(
            "[Groups/0]\nName=Default\nDefault Layout=us\nDefaultIM={}\n\n[Groups/0/Items/0]\nName=keyboard-us\n\n[Groups/0/Items/1]\nName=clak\n\n[GroupOrder]\n0=Default\n",
            default_im
        );
        let _ = fs::write(&profile_path, content);
        return;
    }

    if let Ok(mut text) = fs::read_to_string(&profile_path) {
        if text.contains("DefaultIM=") {
            let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
            for line in &mut lines {
                if line.starts_with("DefaultIM=") {
                    *line = format!("DefaultIM={}", default_im);
                }
            }
            text = lines.join("\n") + "\n";
        }

        if !text.contains("Name=clak") {
            let mut count = 0;
            for line in text.lines() {
                if line.starts_with("[Groups/0/Items/") {
                    count += 1;
                }
            }
            let clak_entry = format!("\n[Groups/0/Items/{}]\nName=clak\n", count);
            if let Some(pos) = text.find("[GroupOrder]") {
                text.insert_str(pos, &clak_entry);
            } else {
                text.push_str(&clak_entry);
            }
        }

        let _ = fs::write(&profile_path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_serialize_deserialize() {
        let cfg = ClakConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
        let parsed: ClakConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(parsed.general.method, "telex");
        assert_eq!(parsed.shortcuts.toggle_vietnamese, "ctrl_shift");
        assert_eq!(parsed.per_app.excluded_apps.len(), 0);
        assert_eq!(parsed.macros.items.len(), 0);
    }

    #[test]
    fn test_custom_config_parse() {
        let raw = r#"
[general]
method = "vni"
short_w = false

[per_app]
remember_state = false
excluded_apps = ["myterminal", "code"]
"#;
        let parsed: ClakConfig = toml::from_str(raw).expect("deserialize");
        assert_eq!(parsed.general.method, "vni");
        assert!(!parsed.general.short_w);
        assert!(!parsed.per_app.remember_state);
        assert_eq!(parsed.per_app.excluded_apps, vec!["myterminal", "code"]);
        assert_eq!(parsed.shortcuts.toggle_vietnamese, "ctrl_shift");
    }
}
