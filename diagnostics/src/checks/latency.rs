use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBenchmark {
    pub uinput_accessible: bool,
    pub uinput_write_us: f64,
    pub measured_rtt_ms: f64,
    pub measured_jitter_ms: f64,
    pub keyboard_repeat_delay_ms: u32,
    pub keyboard_repeat_interval_ms: u32,
    pub scheduler_preempt: String,
    pub system_load: f64,
}

// read keyboard repeat delay and interval from system tools
fn detect_keyboard_repeat() -> (u32, u32) {
    // try hyprctl
    if let Ok(output) = Command::new("hyprctl")
        .args(["getoption", "input:repeat_delay"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            for line in s.lines() {
                if let Some(val_str) = line.trim().strip_prefix("int: ") {
                    if let Ok(delay) = val_str.trim().parse::<u32>() {
                        let rate = Command::new("hyprctl")
                            .args(["getoption", "input:repeat_rate"])
                            .output()
                            .ok()
                            .and_then(|o| {
                                let os = String::from_utf8_lossy(&o.stdout);
                                for l in os.lines() {
                                    if let Some(v) = l.trim().strip_prefix("int: ") {
                                        return v.trim().parse::<u32>().ok();
                                    }
                                }
                                None
                            })
                            .unwrap_or(25);
                        let interval = if rate > 0 { 1000 / rate } else { 40 };
                        return (delay, interval);
                    }
                }
            }
        }
    }

    // try xset q
    if let Ok(output) = Command::new("xset").arg("q").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            let mut delay = 500;
            let mut rate = 25;
            for line in s.lines() {
                if line.contains("auto repeat delay:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    for (i, p) in parts.iter().enumerate() {
                        if *p == "delay:" && i + 1 < parts.len() {
                            delay = parts[i + 1].parse::<u32>().unwrap_or(500);
                        }
                        if *p == "rate:" && i + 1 < parts.len() {
                            rate = parts[i + 1].parse::<u32>().unwrap_or(25);
                        }
                    }
                    let interval = if rate > 0 { 1000 / rate } else { 33 };
                    return (delay, interval);
                }
            }
        }
    }

    // try gsettings
    if let Ok(output) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.peripherals.keyboard", "delay"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            let delay = s
                .trim()
                .trim_start_matches("uint32 ")
                .parse::<u32>()
                .unwrap_or(500);
            let rate_out = Command::new("gsettings")
                .args([
                    "get",
                    "org.gnome.desktop.peripherals.keyboard",
                    "repeat-interval",
                ])
                .output()
                .ok();
            let interval = rate_out
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .trim_start_matches("uint32 ")
                        .parse::<u32>()
                        .unwrap_or(30)
                })
                .unwrap_or(30);
            return (delay, interval);
        }
    }

    (500, 30)
}

// read system load from /proc/loadavg
fn read_system_load() -> f64 {
    if let Ok(content) = fs::read_to_string("/proc/loadavg") {
        if let Some(first) = content.split_whitespace().next() {
            if let Ok(v) = first.parse::<f64>() {
                return v;
            }
        }
    }
    0.0
}

// read kernel preemption mode
fn read_scheduler_preempt() -> String {
    if let Ok(content) = fs::read_to_string("/sys/kernel/debug/sched/preempt") {
        let s = content.trim().to_string();
        if !s.is_empty() {
            return s;
        }
    }
    if let Ok(version) = fs::read_to_string("/proc/version") {
        if version.contains("PREEMPT_DYNAMIC") {
            return "PREEMPT_DYNAMIC".to_string();
        } else if version.contains("PREEMPT_RT") {
            return "PREEMPT_RT".to_string();
        } else if version.contains("PREEMPT") {
            return "PREEMPT".to_string();
        }
    }
    "Standard".to_string()
}

// parse recent [LATENCY] entries from /tmp/clak.log
fn parse_log_latencies() -> Option<(f64, f64)> {
    let content = fs::read_to_string("/tmp/clak.log").ok()?;
    let mut latencies: Vec<f64> = Vec::new();

    for line in content.lines().rev().take(100) {
        if line.contains("[LATENCY]") && line.contains("delta_ms=") {
            if let Some(idx) = line.find("delta_ms=") {
                let sub = &line[idx + 9..];
                let num_str: String = sub
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if let Ok(ms) = num_str.parse::<f64>() {
                    if ms > 0.0 && ms < 500.0 {
                        latencies.push(ms);
                    }
                }
            }
        }
        if latencies.len() >= 20 {
            break;
        }
    }

    if latencies.is_empty() {
        return None;
    }

    let sum: f64 = latencies.iter().sum();
    let avg = sum / (latencies.len() as f64);

    let mut min = f64::MAX;
    let mut max = f64::MIN;
    for &l in &latencies {
        if l < min {
            min = l;
        }
        if l > max {
            max = l;
        }
    }
    let jitter = (max - min).max(0.2);

    Some((avg, jitter))
}

// probe /dev/uinput write latency safely
fn probe_uinput_write() -> (bool, f64) {
    let mut opts = fs::OpenOptions::new();
    opts.write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC);

    let f = match opts.open("/dev/uinput") {
        Ok(file) => file,
        Err(_) => return (false, 0.0),
    };

    use std::os::unix::io::AsRawFd;
    let fd = f.as_raw_fd();

    // measure ioctl setup check latency
    let start = Instant::now();
    unsafe {
        // UI_SET_EVBIT with EV_SYN (value 0) is a safe read/noop check
        libc::ioctl(fd, 0x40045564, 0);
    }
    let elapsed = start.elapsed();
    let write_us = elapsed.as_secs_f64() * 1_000_000.0;

    (true, write_us.max(0.5))
}

pub fn check_latency() -> LatencyBenchmark {
    let (uinput_accessible, uinput_write_us) = probe_uinput_write();
    let (rep_delay, rep_interval) = detect_keyboard_repeat();
    let sys_load = read_system_load();
    let preempt = read_scheduler_preempt();

    let (rtt_ms, jitter_ms) = if let Some((avg, jit)) = parse_log_latencies() {
        (avg, jit)
    } else {
        // baseline estimate: kernel write time + scheduler dispatch overhead
        let base = (uinput_write_us / 1000.0) + 1.8;
        let jit = 0.5 + (sys_load * 0.2);
        (base.max(1.5), jit.min(4.0))
    };

    LatencyBenchmark {
        uinput_accessible,
        uinput_write_us,
        measured_rtt_ms: rtt_ms,
        measured_jitter_ms: jitter_ms,
        keyboard_repeat_delay_ms: rep_delay,
        keyboard_repeat_interval_ms: rep_interval,
        scheduler_preempt: preempt,
        system_load: sys_load,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerStats {
    pub count: u32,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub max_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacingGroupSnapshot {
    pub name: String,
    pub current_delay_ms: u32,
    pub floor_ms: u32,
    pub ceiling_ms: u32,
    pub gap_ms: u32,
    pub hold_us: u32,
    pub ssthresh_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacingTelemetry {
    pub replace_count: u32,
    pub forward_count: u32,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub max_latency_ms: f64,
    pub surrounding_stats: LayerStats,
    pub uinput_stats: LayerStats,
    pub groups: Vec<PacingGroupSnapshot>,
    pub daemon_uptime_seconds: u64,
    pub connected_protocol: String,
    pub active_pacing_mode: String,
    pub drift_detected_count: u32,
    pub sentinel_timeout_count: u32,
    pub overflow_count: u32,
    pub surrounding_mismatch_count: u32,
}

fn compute_layer_stats(mut samples: Vec<f64>) -> LayerStats {
    let count = samples.len() as u32;
    if samples.is_empty() {
        return LayerStats {
            count: 0,
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            max_latency_ms: 0.0,
        };
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = samples.len();
    let p50 = samples[n / 2];
    let p95 = samples[((n as f64 * 0.95).floor() as usize).min(n - 1)];
    let p99 = samples[((n as f64 * 0.99).floor() as usize).min(n - 1)];
    let max_lat = samples[n - 1];
    LayerStats {
        count,
        p50_latency_ms: p50,
        p95_latency_ms: p95,
        p99_latency_ms: p99,
        max_latency_ms: max_lat,
    }
}

fn detect_daemon_uptime_seconds() -> u64 {
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() {
                let cmdline_path = entry.path().join("cmdline");
                if let Ok(content) = fs::read(&cmdline_path) {
                    if let Some(first_null) = content.iter().position(|&b| b == 0) {
                        let arg0 = String::from_utf8_lossy(&content[..first_null]);
                        if std::path::Path::new(&*arg0)
                            .file_name()
                            .is_some_and(|n| n == "fcitx5")
                        {
                            if let Ok(meta) = fs::metadata(format!("/proc/{}", pid)) {
                                if let Ok(mod_time) = meta.modified() {
                                    if let Ok(elapsed) = mod_time.elapsed() {
                                        return elapsed.as_secs();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    0
}

fn detect_connected_protocol() -> String {
    let session = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase();
    let has_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    if session == "wayland" || has_wayland {
        let comp = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase();
        if comp.contains("kde") {
            "Wayland (KWin text-input-v3 / virtual-keyboard-v1)".to_string()
        } else if comp.contains("gnome") {
            "Wayland (GNOME Shell DBus / text-input-v3)".to_string()
        } else if comp.contains("hyprland") || comp.contains("sway") {
            "Wayland (zwp_text_input_v3 / zwp_input_method_v2)".to_string()
        } else {
            "Wayland (zwp_text_input_v3)".to_string()
        }
    } else if std::env::var("DISPLAY").is_ok() {
        "X11 (XIM / DBus Frontend)".to_string()
    } else {
        "DBus IPC (Standalone)".to_string()
    }
}

fn is_adaptive_mode_active(log_content: &str) -> bool {
    if std::env::var("CLAK_ADAPTIVE").ok().as_deref() == Some("1")
        && std::env::var("CLAK_SIMPLE_BACKOFF").is_err()
    {
        return true;
    }
    for line in log_content.lines().rev() {
        if let Some(idx) = line.find("mode=") {
            let sub = &line[idx + 5..];
            let m: String = sub.chars().take_while(|c| !c.is_whitespace()).collect();
            if m == "adaptive" {
                return true;
            } else if !m.is_empty() {
                return false;
            }
        }
    }
    false
}

fn detect_group_snapshots(log_content: &str) -> Vec<PacingGroupSnapshot> {
    let adaptive = is_adaptive_mode_active(log_content);
    let mut snapshots = if adaptive {
        vec![
            PacingGroupSnapshot {
                name: "WebDraftJs (X/Twitter/TikTok/DraftJS)".to_string(),
                current_delay_ms: 16,
                floor_ms: 10,
                ceiling_ms: 40,
                gap_ms: 8,
                hold_us: 1000,
                ssthresh_ms: 10,
            },
            PacingGroupSnapshot {
                name: "Terminal (Kitty/Alacritty/Ghostty)".to_string(),
                current_delay_ms: 12,
                floor_ms: 0,
                ceiling_ms: 30,
                gap_ms: 3,
                hold_us: 1000,
                ssthresh_ms: 0,
            },
            PacingGroupSnapshot {
                name: "HeavyApp (WPS/JetBrains/Steam)".to_string(),
                current_delay_ms: 15,
                floor_ms: 8,
                ceiling_ms: 35,
                gap_ms: 4,
                hold_us: 1000,
                ssthresh_ms: 8,
            },
            PacingGroupSnapshot {
                name: "Default (GTK/Qt/Desktop)".to_string(),
                current_delay_ms: 8,
                floor_ms: 4,
                ceiling_ms: 30,
                gap_ms: 2,
                hold_us: 1000,
                ssthresh_ms: 4,
            },
        ]
    } else {
        vec![
            PacingGroupSnapshot {
                name: "WebDraftJs (X/Twitter/TikTok/DraftJS)".to_string(),
                current_delay_ms: 13,
                floor_ms: 13,
                ceiling_ms: 30,
                gap_ms: 2,
                hold_us: 1000,
                ssthresh_ms: 13,
            },
            PacingGroupSnapshot {
                name: "Terminal (Kitty/Alacritty/Ghostty)".to_string(),
                current_delay_ms: 13,
                floor_ms: 13,
                ceiling_ms: 30,
                gap_ms: 2,
                hold_us: 1000,
                ssthresh_ms: 13,
            },
            PacingGroupSnapshot {
                name: "HeavyApp (WPS/JetBrains/Steam)".to_string(),
                current_delay_ms: 13,
                floor_ms: 13,
                ceiling_ms: 30,
                gap_ms: 2,
                hold_us: 1000,
                ssthresh_ms: 13,
            },
            PacingGroupSnapshot {
                name: "Default (GTK/Qt/Desktop)".to_string(),
                current_delay_ms: 13,
                floor_ms: 13,
                ceiling_ms: 30,
                gap_ms: 2,
                hold_us: 1000,
                ssthresh_ms: 13,
            },
        ]
    };

    for line in log_content.lines() {
        if line.contains("[PACING]") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let mut grp_idx = None;
            let mut post_delay = None;
            let mut ssthresh = None;
            for p in parts {
                if let Some(v) = p.strip_prefix("group=") {
                    grp_idx = v.parse::<usize>().ok();
                } else if let Some(v) = p.strip_prefix("post_delay=") {
                    post_delay = v.trim_end_matches("ms").parse::<u32>().ok();
                } else if let Some(v) = p.strip_prefix("ssthresh=") {
                    ssthresh = v.trim_end_matches("ms").parse::<u32>().ok();
                }
            }
            if let Some(idx) = grp_idx {
                if idx < snapshots.len() {
                    if let Some(pd) = post_delay {
                        snapshots[idx].current_delay_ms = pd;
                    }
                    if let Some(st) = ssthresh {
                        snapshots[idx].ssthresh_ms = st;
                    }
                }
            }
        }
    }
    snapshots
}

pub fn check_telemetry() -> PacingTelemetry {
    let mut all_replaces: Vec<f64> = Vec::new();
    let mut surrounding_replaces: Vec<f64> = Vec::new();
    let mut uinput_replaces: Vec<f64> = Vec::new();
    let mut forwards = 0;
    let mut drifts = 0;
    let mut timeouts = 0;
    let mut overflows = 0;
    let mut mismatches = 0;
    let log_content = fs::read_to_string("/tmp/clak.log").unwrap_or_default();

    for line in log_content.lines() {
        if line.contains("[LATENCY]") {
            if line.contains("action=REPLACE") {
                if let Some(idx) = line.find("delta_ms=") {
                    let sub = &line[idx + 9..];
                    let num_str: String = sub
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if let Ok(ms) = num_str.parse::<f64>() {
                        if ms > 0.0 && ms < 2000.0 {
                            all_replaces.push(ms);
                            if line.contains("layer=surrounding") {
                                surrounding_replaces.push(ms);
                            } else if line.contains("layer=uinput") {
                                uinput_replaces.push(ms);
                            }
                        }
                    }
                }
            } else if line.contains("action=FORWARD") {
                forwards += 1;
            }
        } else {
            let lower = line.to_lowercase();
            if lower.contains("event loop drift") {
                drifts += 1;
            }
            if lower.contains("sentinel timeout") || lower.contains("action=timeout") {
                timeouts += 1;
            }
            if lower.contains("buffer overflow") || lower.contains("dropped old key") {
                overflows += 1;
            }
            if lower.contains("surrounding mismatch") {
                mismatches += 1;
            }
        }
    }

    let overall = compute_layer_stats(all_replaces);
    let surrounding_stats = compute_layer_stats(surrounding_replaces);
    let uinput_stats = compute_layer_stats(uinput_replaces);
    let groups = detect_group_snapshots(&log_content);
    let daemon_uptime_seconds = detect_daemon_uptime_seconds();
    let connected_protocol = detect_connected_protocol();

    let mut active_pacing_mode = if is_adaptive_mode_active(&log_content) {
        "adaptive (CLAK_ADAPTIVE=1)".to_string()
    } else {
        "fixed_13_backoff (mặc định: post 13ms / gap 2ms / pre 13ms, backoff 30ms/4ms)".to_string()
    };
    for line in log_content.lines().rev() {
        if let Some(idx) = line.find("mode=") {
            let sub = &line[idx + 5..];
            let m: String = sub.chars().take_while(|c| !c.is_whitespace()).collect();
            if m == "fixed_13_backoff" {
                active_pacing_mode =
                    "fixed_13_backoff (mặc định: post 13ms / gap 2ms / pre 13ms, backoff 30ms/4ms)"
                        .to_string();
                break;
            } else if m == "adaptive" {
                active_pacing_mode = "adaptive (CLAK_ADAPTIVE=1)".to_string();
                break;
            } else if !m.is_empty() {
                active_pacing_mode = m;
                break;
            }
        }
    }

    PacingTelemetry {
        replace_count: overall.count,
        forward_count: forwards,
        p50_latency_ms: overall.p50_latency_ms,
        p95_latency_ms: overall.p95_latency_ms,
        p99_latency_ms: overall.p99_latency_ms,
        max_latency_ms: overall.max_latency_ms,
        surrounding_stats,
        uinput_stats,
        groups,
        daemon_uptime_seconds,
        connected_protocol,
        active_pacing_mode,
        drift_detected_count: drifts,
        sentinel_timeout_count: timeouts,
        overflow_count: overflows,
        surrounding_mismatch_count: mismatches,
    }
}
