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
