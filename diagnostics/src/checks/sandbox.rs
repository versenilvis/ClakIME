use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChromiumAppImeInfo {
    pub app_name: String,
    pub pid: u32,
    pub ozone_wayland: bool,
    pub has_enable_wayland_ime: bool,
    pub has_text_input_v3: bool,
    pub needs_ime_flag_warning: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlatpakOverrideAssessment {
    pub has_global_override: bool,
    pub has_wayland_or_x11_socket: bool,
    pub has_fcitx_env_or_dbus: bool,
    pub has_ibus_conflict: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxStatus {
    pub chromium_wayland_apps: Vec<ChromiumAppImeInfo>,
    pub flatpak_installed: bool,
    pub flatpak_running_count: usize,
    pub flatpak_apps_detected: Vec<String>,
    pub flatpak_override: FlatpakOverrideAssessment,
    pub snap_running_count: usize,
    pub xdg_desktop_portal_running: bool,
    pub ibus_systemd_respawn_units: Vec<String>,
}

const KNOWN_FLATPAK_APPS: &[&str] = &[
    "com.google.Chrome",
    "com.brave.Browser",
    "org.chromium.Chromium",
    "com.discordapp.Discord",
    "md.obsidian.Obsidian",
    "com.visualstudio.code",
    "org.telegram.desktop",
    "org.mozilla.firefox",
    "com.slack.Slack",
];

// map executable basename to canonical app name without exposing paths
pub fn classify_chromium_binary(exe_name: &str) -> Option<&'static str> {
    let base = exe_name
        .rsplit('/')
        .next()
        .unwrap_or(exe_name)
        .to_ascii_lowercase();
    match base.as_str() {
        "chrome" | "google-chrome" | "google-chrome-stable" | "google-chrome-beta"
        | "google-chrome-unstable" => Some("Google Chrome"),
        "chromium" | "chromium-browser" => Some("Chromium"),
        "brave" | "brave-browser" => Some("Brave"),
        "vivaldi" | "vivaldi-bin" => Some("Vivaldi"),
        "msedge" | "microsoft-edge" => Some("Microsoft Edge"),
        "code" | "code-oss" | "codium" => Some("VS Code"),
        "cursor" => Some("Cursor"),
        "antigravity" | "antigravity-ide" => Some("Antigravity IDE"),
        "obsidian" => Some("Obsidian"),
        "discord" | "vesktop" => Some("Discord"),
        "slack" => Some("Slack"),
        "electron" => Some("Electron"),
        _ => None,
    }
}

// inspect sanitized chromium or electron cmdline flags without retaining args or paths
pub fn inspect_chromium_cmdline(
    pid: u32,
    args: &[&str],
    environ_ozone_hint_wayland: bool,
    on_wayland: bool,
) -> Option<ChromiumAppImeInfo> {
    let first = args.first().copied()?;
    let app_name = classify_chromium_binary(first)?;

    // ignore helper subprocesses such as renderer, gpu, utility, crashpad
    if args.iter().any(|a| a.starts_with("--type=")) {
        return None;
    }

    let mut ozone_wayland = environ_ozone_hint_wayland;
    let mut has_enable_wayland_ime = false;
    let mut has_text_input_v3 = false;

    for arg in args.iter().skip(1) {
        if *arg == "--ozone-platform=wayland"
            || *arg == "--ozone-platform-hint=wayland"
            || *arg == "--ozone-platform-hint=auto"
        {
            ozone_wayland = true;
        } else if *arg == "--enable-wayland-ime" {
            has_enable_wayland_ime = true;
        } else if *arg == "--wayland-text-input-version=3" {
            has_text_input_v3 = true;
        }
    }

    let needs_ime_flag_warning = on_wayland && ozone_wayland && !has_enable_wayland_ime;

    Some(ChromiumAppImeInfo {
        app_name: app_name.to_string(),
        pid,
        ozone_wayland,
        has_enable_wayland_ime,
        has_text_input_v3,
        needs_ime_flag_warning,
    })
}

// parse flatpak override ini content safely
pub fn parse_flatpak_override(content: &str) -> FlatpakOverrideAssessment {
    let mut assessment = FlatpakOverrideAssessment::default();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some(sockets) = line.strip_prefix("sockets=") {
            for tok in sockets.split(';') {
                let t = tok.trim();
                if matches!(t, "wayland" | "x11" | "fallback-x11") {
                    assessment.has_wayland_or_x11_socket = true;
                }
            }
        }
        if line.starts_with("org.fcitx.Fcitx5=")
            || line.starts_with("org.freedesktop.portal.Fcitx=")
            || line.contains("talk-name=org.fcitx.Fcitx5")
            || line.contains("talk-name=org.freedesktop.portal.Fcitx")
        {
            assessment.has_fcitx_env_or_dbus = true;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            if matches!(key, "GTK_IM_MODULE" | "QT_IM_MODULE" | "XMODIFIERS") {
                if val.contains("fcitx") || (key == "GTK_IM_MODULE" && val == "wayland") {
                    assessment.has_fcitx_env_or_dbus = true;
                }
                if val.contains("ibus") {
                    assessment.has_ibus_conflict = true;
                }
            }
        }
    }

    assessment
}

fn check_environ_ozone_hint(environ_bytes: &[u8]) -> bool {
    for entry in environ_bytes.split(|&b| b == 0) {
        if let Ok(s) = std::str::from_utf8(entry) {
            if matches!(
                s,
                "ELECTRON_OZONE_PLATFORM_HINT=auto"
                    | "ELECTRON_OZONE_PLATFORM_HINT=wayland"
                    | "OZONE_PLATFORM=wayland"
            ) {
                return true;
            }
        }
    }
    false
}

fn scan_proc_apps(on_wayland: bool) -> (Vec<ChromiumAppImeInfo>, usize, usize, bool) {
    let mut chromium_apps: Vec<ChromiumAppImeInfo> = Vec::new();
    let mut seen_signatures: BTreeSet<(String, bool, bool, bool)> = BTreeSet::new();
    let mut flatpak_running = 0usize;
    let mut snap_running = 0usize;
    let mut portal_running = false;

    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let pid_str = name.to_string_lossy();
            let Ok(pid) = pid_str.parse::<u32>() else {
                continue;
            };

            let comm = fs::read_to_string(entry.path().join("comm"))
                .unwrap_or_default()
                .trim()
                .to_string();
            if comm == "xdg-desktop-por" || comm == "xdg-desktop-portal" {
                portal_running = true;
            }

            let Ok(cmd_bytes) = fs::read(entry.path().join("cmdline")) else {
                continue;
            };
            if cmd_bytes.is_empty() {
                continue;
            }
            let args: Vec<&str> = cmd_bytes
                .split(|&b| b == 0)
                .filter(|s| !s.is_empty())
                .filter_map(|s| std::str::from_utf8(s).ok())
                .collect();
            if args.is_empty() {
                continue;
            }

            if args[0].contains("xdg-desktop-portal") {
                portal_running = true;
            }
            if args[0].starts_with("/app/bin/") || (comm == "bwrap" && args.iter().any(|a| *a == "/app")) {
                flatpak_running += 1;
            }
            if args[0].starts_with("/snap/") {
                snap_running += 1;
            }

            let has_env_hint = fs::read(entry.path().join("environ"))
                .map(|b| check_environ_ozone_hint(&b))
                .unwrap_or(false);

            if let Some(info) = inspect_chromium_cmdline(pid, &args, has_env_hint, on_wayland) {
                let sig = (
                    info.app_name.clone(),
                    info.ozone_wayland,
                    info.has_enable_wayland_ime,
                    info.has_text_input_v3,
                );
                if seen_signatures.insert(sig) {
                    chromium_apps.push(info);
                }
            }
        }
    }

    (chromium_apps, flatpak_running, snap_running, portal_running)
}

fn collect_flatpak_apps_in_dir(dir: &Path, out: &mut BTreeSet<String>, any_found: &mut bool) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let app_id = entry.file_name().to_string_lossy().to_string();
            if !app_id.starts_with('.') {
                *any_found = true;
                if KNOWN_FLATPAK_APPS.contains(&app_id.as_str()) {
                    out.insert(app_id);
                }
            }
        }
    }
}

pub fn check_systemd_user_ibus_units_in(user_systemd_dir: &Path) -> Vec<String> {
    let mut found = BTreeSet::new();
    let targets = [
        "org.freedesktop.IBus.session.GNOME.service",
        "org.freedesktop.IBus.session.generic.service",
        "ibus.service",
    ];

    if let Ok(entries) = fs::read_dir(user_systemd_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if !fname.ends_with(".wants") {
                continue;
            }
            if let Ok(links) = fs::read_dir(entry.path()) {
                for link in links.flatten() {
                    let lname = link.file_name().to_string_lossy().to_string();
                    if lname.to_ascii_lowercase().contains("ibus") {
                        found.insert(lname);
                    }
                }
            }
        }
    }

    // also inspect active systemd user units read-only
    for unit in targets {
        if let Ok(out) = Command::new("systemctl")
            .args(["--user", "is-active", unit])
            .output()
        {
            let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if state == "active" || state == "activating" {
                found.insert(unit.to_string());
            }
        }
    }

    found.into_iter().collect()
}

pub fn check_sandbox_status() -> SandboxStatus {
    let on_wayland = env::var("XDG_SESSION_TYPE")
        .map(|s| s == "wayland")
        .unwrap_or(false)
        || env::var("WAYLAND_DISPLAY").is_ok();

    let (chromium_wayland_apps, flatpak_running_count, snap_running_count, xdg_desktop_portal_running) =
        scan_proc_apps(on_wayland);

    let home = env::var("HOME").unwrap_or_default();
    let user_flatpak_app = PathBuf::from(&home).join(".local/share/flatpak/app");
    let sys_flatpak_app = Path::new("/var/lib/flatpak/app");
    let user_global_override = PathBuf::from(&home).join(".local/share/flatpak/overrides/global");

    let mut flatpak_installed = false;
    let mut detected_apps = BTreeSet::new();
    if !home.is_empty() {
        collect_flatpak_apps_in_dir(&user_flatpak_app, &mut detected_apps, &mut flatpak_installed);
    }
    collect_flatpak_apps_in_dir(sys_flatpak_app, &mut detected_apps, &mut flatpak_installed);

    let mut flatpak_override = FlatpakOverrideAssessment::default();
    if !home.is_empty() && user_global_override.exists() {
        if let Ok(content) = fs::read_to_string(&user_global_override) {
            flatpak_override = parse_flatpak_override(&content);
            flatpak_override.has_global_override = true;
        }
    }

    let user_systemd_dir = PathBuf::from(&home).join(".config/systemd/user");
    let ibus_systemd_respawn_units = if home.is_empty() {
        Vec::new()
    } else {
        check_systemd_user_ibus_units_in(&user_systemd_dir)
    };

    SandboxStatus {
        chromium_wayland_apps,
        flatpak_installed: flatpak_installed || flatpak_running_count > 0,
        flatpak_running_count,
        flatpak_apps_detected: detected_apps.into_iter().collect(),
        flatpak_override,
        snap_running_count,
        xdg_desktop_portal_running,
        ibus_systemd_respawn_units,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chromium_cmdline_flags_detection_and_helper_filter() {
        // helper process must be skipped
        assert!(inspect_chromium_cmdline(
            100,
            &["/usr/lib/chromium/chromium", "--type=renderer", "--ozone-platform=wayland"],
            false,
            true
        )
        .is_none());

        // main process on wayland with ozone-wayland but missing --enable-wayland-ime triggers warning
        let missing_ime = inspect_chromium_cmdline(
            101,
            &[
                "/opt/google/chrome/google-chrome",
                "--ozone-platform=wayland",
                "https://private.example.com/secret-doc",
            ],
            false,
            true,
        )
        .unwrap();
        assert_eq!(missing_ime.app_name, "Google Chrome");
        assert!(missing_ime.ozone_wayland);
        assert!(!missing_ime.has_enable_wayland_ime);
        assert!(missing_ime.needs_ime_flag_warning);

        // electron app using ELECTRON_OZONE_PLATFORM_HINT=auto with --enable-wayland-ime passes cleanly
        let ok_electron = inspect_chromium_cmdline(
            102,
            &["/usr/bin/obsidian", "--enable-wayland-ime", "--wayland-text-input-version=3"],
            true,
            true,
        )
        .unwrap();
        assert_eq!(ok_electron.app_name, "Obsidian");
        assert!(ok_electron.ozone_wayland);
        assert!(ok_electron.has_enable_wayland_ime);
        assert!(ok_electron.has_text_input_v3);
        assert!(!ok_electron.needs_ime_flag_warning);

        // privacy check: private url must never appear in serialized struct
        let serialized = serde_json::to_string(&missing_ime).unwrap();
        assert!(!serialized.contains("private.example.com"));
        assert!(!serialized.contains("/opt/google"));
    }

    #[test]
    fn test_flatpak_override_parsing() {
        let good = "[Context]\nsockets=wayland;x11;\n[Environment]\nXMODIFIERS=@im=fcitx\nQT_IM_MODULE=fcitx\n";
        let res = parse_flatpak_override(good);
        assert!(res.has_wayland_or_x11_socket);
        assert!(res.has_fcitx_env_or_dbus);
        assert!(!res.has_ibus_conflict);

        let ibus = "[Environment]\nGTK_IM_MODULE=ibus\n";
        let bad = parse_flatpak_override(ibus);
        assert!(bad.has_ibus_conflict);
    }

    #[test]
    fn test_systemd_user_ibus_wants_detection() {
        let tmp = std::env::temp_dir().join(format!("clak_systemd_ibus_{}", std::process::id()));
        let wants = tmp.join("default.target.wants");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&wants).unwrap();
        fs::write(wants.join("org.freedesktop.IBus.session.GNOME.service"), "").unwrap();

        let units = check_systemd_user_ibus_units_in(&tmp);
        let _ = fs::remove_dir_all(&tmp);

        assert!(units.iter().any(|u| u.contains("IBus")));
    }
}
