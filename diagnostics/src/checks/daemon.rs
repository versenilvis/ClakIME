use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub fcitx5_running: bool,
    pub fcitx5_pids: Vec<u32>,
    pub current_im: Option<String>,
}

pub fn check_daemon() -> DaemonStatus {
    let mut fcitx5_pids = Vec::new();

    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if let Ok(pid) = name_str.parse::<u32>() {
                let cmdline_path = entry.path().join("cmdline");
                if let Ok(content) = fs::read(&cmdline_path) {
                    if let Some(first_null) = content.iter().position(|&b| b == 0) {
                        let arg0 = String::from_utf8_lossy(&content[..first_null]);
                        if Path::new(&*arg0)
                            .file_name()
                            .map_or(false, |n| n == "fcitx5")
                        {
                            fcitx5_pids.push(pid);
                        }
                    }
                }
            }
        }
    }

    let fcitx5_running = !fcitx5_pids.is_empty();

    let current_im = if fcitx5_running {
        Command::new("fcitx5-remote")
            .arg("-n")
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    let im = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !im.is_empty() {
                        Some(im)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
    } else {
        None
    };

    DaemonStatus {
        fcitx5_running,
        fcitx5_pids,
        current_im,
    }
}
