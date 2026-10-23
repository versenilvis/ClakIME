use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f64,
    pub is_active_focus: bool,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayEnvironment {
    pub monitors: Vec<MonitorInfo>,
    pub effective_refresh_rate: f64,
    pub frame_budget_ms: f64,
    pub is_mixed_refresh_rate: bool,
    pub detection_method: String,
}

// query hyprland monitors and active window via hyprctl
fn detect_hyprland() -> Option<Vec<MonitorInfo>> {
    let output = Command::new("hyprctl")
        .args(["-j", "monitors"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let val: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let arr = val.as_array()?;

    // query active window to identify active monitor
    let active_mon_name = Command::new("hyprctl")
        .args(["-j", "activewindow"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
        .and_then(|v| {
            v.get("monitor")
                .and_then(|m| m.as_str().map(|s| s.to_string()))
                .or_else(|| {
                    v.get("monitor")
                        .and_then(|m| m.as_i64().map(|id| id.to_string()))
                })
        });

    let mut monitors = Vec::new();
    for (idx, item) in arr.iter().enumerate() {
        let name = item
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let width = item.get("width").and_then(|w| w.as_u64()).unwrap_or(1920) as u32;
        let height = item.get("height").and_then(|h| h.as_u64()).unwrap_or(1080) as u32;
        let rate = item
            .get("refreshRate")
            .and_then(|r| r.as_f64())
            .unwrap_or(60.0);
        let focused = item
            .get("focused")
            .and_then(|f| f.as_bool())
            .unwrap_or(false);

        let is_active = if let Some(ref target) = active_mon_name {
            target == &name || target == &idx.to_string()
        } else {
            focused
        };

        monitors.push(MonitorInfo {
            name,
            width,
            height,
            refresh_rate: rate,
            is_active_focus: is_active,
            is_primary: idx == 0,
        });
    }

    if monitors.is_empty() {
        None
    } else {
        Some(monitors)
    }
}

// query kde kscreen-doctor and active output name
fn detect_kde() -> Option<Vec<MonitorInfo>> {
    let output = Command::new("kscreen-doctor").arg("-j").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let val: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let outputs = val.get("outputs")?.as_array()?;

    // get active output name from kwin via qdbus6 or qdbus if available
    let active_output = Command::new("qdbus6")
        .args(["org.kde.KWin", "/KWin", "org.kde.KWin.activeOutputName"])
        .output()
        .or_else(|_| {
            Command::new("qdbus")
                .args(["org.kde.KWin", "/KWin", "org.kde.KWin.activeOutputName"])
                .output()
        })
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            } else {
                None
            }
        });

    let mut monitors = Vec::new();
    for item in outputs {
        let connected = item
            .get("connected")
            .and_then(|c| c.as_bool())
            .unwrap_or(false);
        let enabled = item
            .get("enabled")
            .and_then(|e| e.as_bool())
            .unwrap_or(false);
        if !connected || !enabled {
            continue;
        }

        let name = item
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let current_mode_id = item
            .get("currentModeId")
            .and_then(|m| m.as_str())
            .unwrap_or("");

        let mut rate = 60.0;
        let mut width = 1920;
        let mut height = 1080;

        if let Some(modes) = item.get("modes").and_then(|m| m.as_array()) {
            for m in modes {
                let mid = m.get("id").and_then(|i| i.as_str()).unwrap_or("");
                if mid == current_mode_id {
                    rate = m
                        .get("refreshRate")
                        .and_then(|r| r.as_f64())
                        .unwrap_or(60.0);
                    if let Some(sz) = m.get("size") {
                        width = sz.get("width").and_then(|w| w.as_u64()).unwrap_or(1920) as u32;
                        height = sz.get("height").and_then(|h| h.as_u64()).unwrap_or(1080) as u32;
                    }
                    break;
                }
            }
        }

        let is_active = active_output.as_ref() == Some(&name);
        let priority = item.get("priority").and_then(|p| p.as_u64()).unwrap_or(1);

        monitors.push(MonitorInfo {
            name,
            width,
            height,
            refresh_rate: rate,
            is_active_focus: is_active,
            is_primary: priority == 0,
        });
    }

    if monitors.is_empty() {
        None
    } else {
        Some(monitors)
    }
}

// query gnome mutter display config via gdbus
fn detect_gnome() -> Option<Vec<MonitorInfo>> {
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Mutter.DisplayConfig",
            "--object-path",
            "/org/gnome/Mutter/DisplayConfig",
            "--method",
            "org.gnome.Mutter.DisplayConfig.GetCurrentState",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let out_str = String::from_utf8_lossy(&output.stdout);
    let mut monitors = Vec::new();

    // parse monitors from mutter gdbus text representation
    for chunk in out_str.split("('") {
        if let Some(conn_end) = chunk.find('\'') {
            let conn_name = &chunk[..conn_end];
            if conn_name.starts_with("eDP")
                || conn_name.starts_with("DP")
                || conn_name.starts_with("HDMI")
            {
                let mut rate = 60.0;
                let mut width = 1920;
                let mut height = 1080;
                if let Some(mode_start) = chunk.find('@') {
                    let sub = &chunk[mode_start + 1..];
                    let rate_str: String = sub
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if let Ok(r) = rate_str.parse::<f64>() {
                        if r > 20.0 {
                            rate = r;
                        }
                    }
                }
                if let Some(res_start) = chunk.find('x') {
                    let before: String = chunk[..res_start]
                        .chars()
                        .rev()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    let before_str: String = before.chars().rev().collect();
                    let after: String = chunk[res_start + 1..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let (Ok(w), Ok(h)) = (before_str.parse::<u32>(), after.parse::<u32>()) {
                        width = w;
                        height = h;
                    }
                }
                monitors.push(MonitorInfo {
                    name: conn_name.to_string(),
                    width,
                    height,
                    refresh_rate: rate,
                    is_active_focus: false,
                    is_primary: monitors.is_empty(),
                });
            }
        }
    }

    if monitors.is_empty() {
        None
    } else {
        Some(monitors)
    }
}

// universal fallback parsing /sys/class/drm/
fn detect_drm_fallback() -> Option<Vec<MonitorInfo>> {
    let entries = fs::read_dir("/sys/class/drm").ok()?;
    let mut monitors = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let status_path = path.join("status");
        if let Ok(status) = fs::read_to_string(&status_path) {
            if status.trim() == "connected" {
                let name = path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("unknown")
                    .trim_start_matches("card0-")
                    .trim_start_matches("card1-")
                    .to_string();

                let modes_path = path.join("modes");
                let mut width = 1920;
                let mut height = 1080;
                if let Ok(modes) = fs::read_to_string(&modes_path) {
                    if let Some(first_line) = modes.lines().next() {
                        if let Some((w_s, h_s)) = first_line.trim().split_once('x') {
                            if let (Ok(w), Ok(h)) = (w_s.parse::<u32>(), h_s.parse::<u32>()) {
                                width = w;
                                height = h;
                            }
                        }
                    }
                }

                monitors.push(MonitorInfo {
                    name,
                    width,
                    height,
                    refresh_rate: 60.0,
                    is_active_focus: false,
                    is_primary: monitors.is_empty(),
                });
            }
        }
    }

    if monitors.is_empty() {
        None
    } else {
        Some(monitors)
    }
}

// inspect multi-monitor environment and calculate effective refresh rate
pub fn check_display() -> DisplayEnvironment {
    let (monitors, method) = if let Some(m) = detect_hyprland() {
        (m, "Hyprland IPC".to_string())
    } else if let Some(m) = detect_kde() {
        (m, "KDE KScreen".to_string())
    } else if let Some(m) = detect_gnome() {
        (m, "GNOME Mutter".to_string())
    } else if let Some(m) = detect_drm_fallback() {
        (m, "Linux DRM sysfs".to_string())
    } else {
        (
            vec![MonitorInfo {
                name: "default".to_string(),
                width: 1920,
                height: 1080,
                refresh_rate: 60.0,
                is_active_focus: true,
                is_primary: true,
            }],
            "Standard Fallback".to_string(),
        )
    };

    let mut is_mixed = false;
    let mut min_rate = f64::MAX;
    let mut max_rate = f64::MIN;
    let mut active_rate = None;

    for m in &monitors {
        if m.refresh_rate < min_rate {
            min_rate = m.refresh_rate;
        }
        if m.refresh_rate > max_rate {
            max_rate = m.refresh_rate;
        }
        if m.is_active_focus {
            active_rate = Some(m.refresh_rate);
        }
    }

    if monitors.len() > 1 && (max_rate - min_rate).abs() > 5.0 {
        is_mixed = true;
    }

    // if an active window monitor is identified, use its refresh rate
    // otherwise apply fail-safe floor: choose the minimum refresh rate to guarantee typing correctness
    let effective_rate = if let Some(act) = active_rate {
        act
    } else if min_rate > 20.0 && min_rate < f64::MAX {
        min_rate
    } else {
        60.0
    };

    let frame_budget_ms = if effective_rate > 0.0 {
        1000.0 / effective_rate
    } else {
        16.67
    };

    DisplayEnvironment {
        monitors,
        effective_refresh_rate: effective_rate,
        frame_budget_ms,
        is_mixed_refresh_rate: is_mixed,
        detection_method: method,
    }
}
