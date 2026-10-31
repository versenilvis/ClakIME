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

// clean up ibus conflicts and enforce fcitx5 environment
pub fn fix_environment_conflicts() -> Result<(), String> {
    let home = env::var("HOME").map_err(|e| e.to_string())?;
    let home_path = PathBuf::from(&home);

    // switch debian/ubuntu system input method to fcitx5
    let _ = std::process::Command::new("im-config")
        .args(["-n", "fcitx5"])
        .status();

    // write fallback xinputrc for x11 sessions
    let xinputrc_file = home_path.join(".xinputrc");
    let xinputrc_content = "# im-config sign\nrun_im fcitx5\n";
    let _ = fs::write(&xinputrc_file, xinputrc_content);

    // ensure environment.d config exists without gtk_im_module override
    let env_dir = home_path.join(".config/environment.d");
    fs::create_dir_all(&env_dir).map_err(|e| e.to_string())?;
    let env_file = env_dir.join("99-clak-im.conf");

    let env_content = "QT_IM_MODULE=fcitx\n\
QT_IM_MODULES=\"wayland;fcitx\"\n\
XMODIFIERS=@im=fcitx\n\
INPUT_METHOD=fcitx5\n\
SDL_IM_MODULE=fcitx\n";

    fs::write(&env_file, env_content).map_err(|e| e.to_string())?;

    // strip ibus overrides from shell configuration files
    let shell_rc_files = [
        home_path.join(".profile"),
        home_path.join(".xprofile"),
        home_path.join(".bashrc"),
        home_path.join(".zshrc"),
        home_path.join(".pam_environment"),
    ];

    for file_path in &shell_rc_files {
        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(file_path) {
                if content.contains("GTK_IM_MODULE=ibus")
                    || content.contains("QT_IM_MODULE=ibus")
                    || content.contains("XMODIFIERS=@im=ibus")
                {
                    let cleaned: Vec<String> = content
                        .lines()
                        .filter(|l| {
                            let trimmed = l.trim();
                            !trimmed.starts_with("export GTK_IM_MODULE=ibus")
                                && !trimmed.starts_with("GTK_IM_MODULE=ibus")
                                && !trimmed.starts_with("export QT_IM_MODULE=ibus")
                                && !trimmed.starts_with("QT_IM_MODULE=ibus")
                                && !trimmed.starts_with("export XMODIFIERS=@im=ibus")
                                && !trimmed.starts_with("XMODIFIERS=@im=ibus")
                        })
                        .map(|s| s.to_string())
                        .collect();
                    let _ = fs::write(file_path, cleaned.join("\n") + "\n");
                }
            }
        }
    }

    // clean ibus overrides from environment.d files
    if let Ok(entries) = fs::read_dir(&env_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path != env_file
                && path.extension().is_some_and(|ext| ext == "conf")
            {
                if let Ok(content) = fs::read_to_string(&path) {
                    if content.contains("GTK_IM_MODULE=ibus")
                        || content.contains("QT_IM_MODULE=ibus")
                    {
                        let cleaned: Vec<String> = content
                            .lines()
                            .filter(|l| {
                                let trimmed = l.trim();
                                !trimmed.starts_with("GTK_IM_MODULE=ibus")
                                    && !trimmed.starts_with("export GTK_IM_MODULE=ibus")
                                    && !trimmed.starts_with("QT_IM_MODULE=ibus")
                                    && !trimmed.starts_with("export QT_IM_MODULE=ibus")
                            })
                            .map(|s| s.to_string())
                            .collect();
                        let _ = fs::write(&path, cleaned.join("\n") + "\n");
                    }
                }
            }
        }
    }

    // clear gtk_im_module and update qt/xmodifiers in systemd user session
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "unset-environment", "GTK_IM_MODULE"])
        .status();
    let _ = std::process::Command::new("systemctl")
        .args([
            "--user",
            "set-environment",
            "QT_IM_MODULE=fcitx",
            "QT_IM_MODULES=wayland;fcitx",
            "XMODIFIERS=@im=fcitx",
            "INPUT_METHOD=fcitx5",
            "SDL_IM_MODULE=fcitx",
        ])
        .status();

    // sync dbus activation environment
    let _ = std::process::Command::new("dbus-update-activation-environment")
        .args(["--systemd", "GTK_IM_MODULE="])
        .status();
    let _ = std::process::Command::new("dbus-update-activation-environment")
        .args([
            "--systemd",
            "QT_IM_MODULE=fcitx",
            "QT_IM_MODULES=wayland;fcitx",
            "XMODIFIERS=@im=fcitx",
            "INPUT_METHOD=fcitx5",
            "SDL_IM_MODULE=fcitx",
        ])
        .status();

    // update current process environment for immediate diagnostic scan
    std::env::remove_var("GTK_IM_MODULE");
    std::env::set_var("QT_IM_MODULE", "fcitx");
    std::env::set_var("XMODIFIERS", "@im=fcitx");
    std::env::set_var("INPUT_METHOD", "fcitx5");

    Ok(())
}

pub fn enable_autostart() -> Result<(), String> {
    let home = env::var("HOME").map_err(|e| e.to_string())?;
    let home_path = PathBuf::from(home);

    let autostart_dir = home_path.join(".config/autostart");
    fs::create_dir_all(&autostart_dir).map_err(|e| e.to_string())?;
    let desktop_file = autostart_dir.join("clak-autostart.desktop");

    let desktop_content = "[Desktop Entry]\n\
Type=Application\n\
Name=Clak Vietnamese Input Method\n\
Comment=Autostart Fcitx5 with Clak input method\n\
Exec=fcitx5 -d\n\
Icon=org.fcitx.Fcitx5\n\
Terminal=false\n\
Categories=System;Utility;\n\
StartupNotify=false\n\
X-GNOME-Autostart-Phase=Applications\n\
X-GNOME-AutoRestart=true\n\
X-GNOME-Autostart-Notify=false\n\
X-KDE-autostart-after=panel\n";

    let _ = fs::write(&desktop_file, desktop_content).map_err(|e| e.to_string())?;

    // ensure environment variables and fallback scripts are configured
    let _ = fix_environment_conflicts();

    let env_dir = home_path.join(".config/environment.d");
    fs::create_dir_all(&env_dir).map_err(|e| e.to_string())?;
    let env_file = env_dir.join("99-clak-im.conf");

    let env_content = "QT_IM_MODULE=fcitx\n\
QT_IM_MODULES=\"wayland;fcitx\"\n\
XMODIFIERS=@im=fcitx\n\
INPUT_METHOD=fcitx5\n\
SDL_IM_MODULE=fcitx\n";

    fs::write(&env_file, env_content).map_err(|e| e.to_string())?;

    // sync environment variables to systemd user session and dbus activation
    let _ = std::process::Command::new("systemctl")
        .args([
            "--user",
            "set-environment",
            "QT_IM_MODULE=fcitx",
            "QT_IM_MODULES=wayland;fcitx",
            "XMODIFIERS=@im=fcitx",
            "INPUT_METHOD=fcitx5",
            "SDL_IM_MODULE=fcitx",
        ])
        .status();
    let _ = std::process::Command::new("dbus-update-activation-environment")
        .args([
            "--systemd",
            "QT_IM_MODULE=fcitx",
            "QT_IM_MODULES=wayland;fcitx",
            "XMODIFIERS=@im=fcitx",
            "INPUT_METHOD=fcitx5",
            "SDL_IM_MODULE=fcitx",
        ])
        .status();

    let profile_file = home_path.join(".config/fcitx5/profile");
    if profile_file.exists() {
        if let Ok(mut prof) = fs::read_to_string(&profile_file) {
            if !prof.contains("Name=clak") {
                let count = prof.matches("[Groups/0/Items/").count();
                let clak_entry = format!("\n[Groups/0/Items/{}]\nName=clak\n", count);
                if let Some(idx) = prof.find("[GroupOrder]") {
                    prof.insert_str(idx, &clak_entry);
                } else {
                    prof.push_str(&clak_entry);
                }
            }

            let lines: Vec<String> = prof
                .lines()
                .map(|line| {
                    if line.starts_with("DefaultIM=") {
                        "DefaultIM=clak".to_string()
                    } else {
                        line.to_string()
                    }
                })
                .collect();

            let new_prof = lines.join("\n") + "\n";
            let _ = fs::write(&profile_file, new_prof);
        }
    }

    Ok(())
}

pub fn disable_autostart() -> Result<(), String> {
    let home = env::var("HOME").map_err(|e| e.to_string())?;
    let home_path = PathBuf::from(home);

    let desktop_file = home_path.join(".config/autostart/clak-autostart.desktop");
    if desktop_file.exists() {
        let _ = fs::remove_file(desktop_file);
    }

    let env_file = home_path.join(".config/environment.d/99-clak-im.conf");
    if env_file.exists() {
        let _ = fs::remove_file(env_file);
    }

    Ok(())
}
