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

fn extract_first_quote(s: &str) -> Option<String> {
    let start = s.find('"')?;
    let end = s[start + 1..].find('"')?;
    let val = s[start + 1..start + 1 + end].trim().to_string();
    if !val.is_empty() {
        Some(val)
    } else {
        None
    }
}

fn extract_second_quote(s: &str) -> Option<String> {
    let mut count = 0;
    let mut in_quote = false;
    let mut cur = String::new();
    let mut escaped = false;
    for ch in s.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            if in_quote {
                count += 1;
                if count == 2 {
                    let val = cur.trim().to_string();
                    return if !val.is_empty() { Some(val) } else { None };
                }
                cur.clear();
            }
            in_quote = !in_quote;
        } else if in_quote {
            cur.push(ch);
        }
    }
    None
}

fn get_group_default_im() -> Option<String> {
    let group = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.fcitx.Fcitx5",
            "/controller",
            "org.fcitx.Fcitx.Controller1",
            "CurrentInputMethodGroup",
        ])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                extract_first_quote(&s)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "Default".to_string());

    Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.fcitx.Fcitx5",
            "/controller",
            "org.fcitx.Fcitx.Controller1",
            "FullInputMethodGroupInfo",
            "s",
            &group,
        ])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                extract_second_quote(&s)
            } else {
                None
            }
        })
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
                        if Path::new(&*arg0).file_name().is_some_and(|n| n == "fcitx5") {
                            fcitx5_pids.push(pid);
                        }
                    }
                }
            }
        }
    }

    let fcitx5_running = !fcitx5_pids.is_empty();

    let current_im = if fcitx5_running {
        let active_im = Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.fcitx.Fcitx5",
                "/controller",
                "org.fcitx.Fcitx.Controller1",
                "CurrentInputMethod",
            ])
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    extract_first_quote(&s)
                } else {
                    None
                }
            })
            .or_else(|| {
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
            });

        // on wayland/dbus, currentinputmethod returns layout fallback when unfocused
        if active_im.as_deref() == Some("clak") {
            Some("clak".to_string())
        } else if let Some(group_im) = get_group_default_im() {
            Some(group_im)
        } else {
            active_im
        }
    } else {
        None
    };

    DaemonStatus {
        fcitx5_running,
        fcitx5_pids,
        current_im,
    }
}
