use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::CStr;
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub kernel_release: String,
    pub architecture: String,
    pub session_type: String,
    pub desktop_environment: String,
    pub env_key: String,
}

fn detect_fcitx5_version() -> String {
    if let Ok(output) = Command::new("fcitx5").arg("--version").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            if let Some(v) = s.lines().next() {
                let trimmed = v.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

fn detect_compositor_version(comp: &str) -> String {
    match comp {
        "hyprland" => {
            if let Ok(output) = Command::new("hyprctl").arg("version").output() {
                if output.status.success() {
                    let s = String::from_utf8_lossy(&output.stdout);
                    if let Some(line) = s.lines().next() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let ver = parts[1];
                            let mut commit = "";
                            if let Some(pos) = parts.iter().position(|&x| x == "commit") {
                                if pos + 1 < parts.len() {
                                    commit = parts[pos + 1];
                                }
                            }
                            if !commit.is_empty() {
                                let short_commit: String = commit.chars().take(8).collect();
                                return format!("{}({})", ver, short_commit);
                            }
                            return ver.to_string();
                        }
                    }
                }
            }
            if let Ok(sig) = env::var("HYPRLAND_INSTANCE_SIGNATURE") {
                if let Some((commit, _)) = sig.split_once('_') {
                    let short: String = commit.chars().take(8).collect();
                    return format!("unknown({})", short);
                }
            }
            "unknown".to_string()
        }
        "sway" => {
            if let Ok(output) = Command::new("sway").arg("--version").output() {
                if output.status.success() {
                    let s = String::from_utf8_lossy(&output.stdout);
                    if let Some(line) = s.lines().next() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 {
                            return parts[2].to_string();
                        }
                    }
                }
            }
            "1.x".to_string()
        }
        "gnome" => {
            if let Ok(output) = Command::new("gnome-shell").arg("--version").output() {
                if output.status.success() {
                    let s = String::from_utf8_lossy(&output.stdout);
                    if let Some(line) = s.lines().next() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 {
                            return parts[2].to_string();
                        }
                    }
                }
            }
            "unknown".to_string()
        }
        "kde" => {
            if let Ok(output) = Command::new("plasmashell").arg("--version").output() {
                if output.status.success() {
                    let s = String::from_utf8_lossy(&output.stdout);
                    if let Some(line) = s.lines().next() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            return parts[1].to_string();
                        }
                    }
                }
            }
            env::var("KDE_SESSION_VERSION").unwrap_or_else(|_| "unknown".to_string())
        }
        _ => "unknown".to_string(),
    }
}

fn detect_env_key() -> String {
    let mut comp = "unknown".to_string();

    let hypr_sig = env::var("HYPRLAND_INSTANCE_SIGNATURE").ok();
    let sway_sock = env::var("SWAYSOCK").ok();
    let env_comp = env::var("XDG_CURRENT_DESKTOP").ok();

    if let Some(sig) = hypr_sig {
        if !sig.is_empty() {
            comp = "hyprland".to_string();
        }
    } else if let Some(sock) = sway_sock {
        if !sock.is_empty() {
            comp = "sway".to_string();
        }
    } else if let Some(c) = env_comp {
        if !c.is_empty() {
            comp = c.to_lowercase();
        }
    } else if env::var("WAYLAND_DISPLAY").is_ok() {
        comp = "wayland".to_string();
    } else if env::var("DISPLAY").is_ok() {
        comp = "x11".to_string();
    }

    let comp_ver = detect_compositor_version(&comp);
    let fcitx5_ver = detect_fcitx5_version();
    let clak_ver = clak_engine::VERSION;

    format!("{}:{}:fcitx5:{}:clak:{}", comp, comp_ver, fcitx5_ver, clak_ver)
}

pub fn check_system() -> SystemInfo {
    let mut os_name = String::new();
    let mut kernel_release = String::new();
    let mut architecture = String::new();

    unsafe {
        let mut uts: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut uts) == 0 {
            os_name = CStr::from_ptr(uts.sysname.as_ptr())
                .to_string_lossy()
                .into_owned();
            kernel_release = CStr::from_ptr(uts.release.as_ptr())
                .to_string_lossy()
                .into_owned();
            architecture = CStr::from_ptr(uts.machine.as_ptr())
                .to_string_lossy()
                .into_owned();
        }
    }

    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                let pretty = val.trim_matches('"').trim();
                if !pretty.is_empty() {
                    os_name = pretty.to_string();
                }
                break;
            }
        }
    }

    let session_type = env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".to_string());
    let desktop_environment =
        env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".to_string());
    let env_key = detect_env_key();

    SystemInfo {
        os_name,
        kernel_release,
        architecture,
        session_type,
        desktop_environment,
        env_key,
    }
}
