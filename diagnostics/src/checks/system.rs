use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::CStr;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub kernel_release: String,
    pub architecture: String,
    pub session_type: String,
    pub desktop_environment: String,
    pub env_key: String,
}

fn detect_env_key() -> String {
    let mut comp = "unknown".to_string();
    let mut comp_ver = "unknown".to_string();

    let hypr_sig = env::var("HYPRLAND_INSTANCE_SIGNATURE").ok();
    let sway_sock = env::var("SWAYSOCK").ok();
    let env_comp = env::var("XDG_CURRENT_DESKTOP").ok();
    let kde_ver = env::var("KDE_SESSION_VERSION").ok();

    if let Some(sig) = hypr_sig {
        if !sig.is_empty() {
            comp = "hyprland".to_string();
            if let Some((commit, _)) = sig.split_once('_') {
                comp_ver = commit.chars().take(12).collect();
            }
        }
    } else if let Some(sock) = sway_sock {
        if !sock.is_empty() {
            comp = "sway".to_string();
            comp_ver = "1.x".to_string();
        }
    } else if let Some(c) = env_comp {
        if !c.is_empty() {
            comp = c.to_lowercase();
            if let Some(kv) = kde_ver {
                if !kv.is_empty() {
                    comp_ver = kv;
                }
            }
        }
    } else if env::var("WAYLAND_DISPLAY").is_ok() {
        comp = "wayland".to_string();
    } else if env::var("DISPLAY").is_ok() {
        comp = "x11".to_string();
    }

    format!("{}:{}:0.7.8:fcitx5", comp, comp_ver)
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
