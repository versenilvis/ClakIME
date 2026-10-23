use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStatus {
    pub profile_exists: bool,
    pub profile_path: PathBuf,
    pub clak_in_profile: bool,
    pub is_default_im: bool,
    pub default_layout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WpsStatus {
    pub installed: bool,
    pub configured: bool,
    pub desktop_files_patched: usize,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JetBrainsStatus {
    pub installed: bool,
    pub unconfigured_ides: Vec<String>,
    pub configured_ides: Vec<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdeStatus {
    pub is_kde: bool,
    pub virtual_keyboard_configured: bool,
    pub input_method_entry: Option<String>,
    pub explanation: String,
}

// inspect fcitx5 profile configuration for clak presence
pub fn check_profile() -> ProfileStatus {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let profile_path = PathBuf::from(&home).join(".config/fcitx5/profile");

    if !profile_path.exists() {
        return ProfileStatus {
            profile_exists: false,
            profile_path,
            clak_in_profile: false,
            is_default_im: false,
            default_layout: None,
        };
    }

    let content = fs::read_to_string(&profile_path).unwrap_or_default();
    let mut clak_in_profile = false;
    let mut is_default_im = false;
    let mut default_layout = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "Name=clak" {
            clak_in_profile = true;
        } else if trimmed.starts_with("DefaultIM=") {
            let val = trimmed.trim_start_matches("DefaultIM=").trim();
            if val == "clak" {
                is_default_im = true;
            }
        } else if trimmed.starts_with("Default Layout=") {
            let val = trimmed.trim_start_matches("Default Layout=").trim();
            default_layout = Some(val.to_string());
        }
    }

    ProfileStatus {
        profile_exists: true,
        profile_path,
        clak_in_profile,
        is_default_im,
        default_layout,
    }
}

// get active input method group name from fcitx5 dbus, falling back to "Default"
fn get_active_im_group() -> String {
    let out = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.fcitx.Fcitx5",
            "/controller",
            "org.fcitx.Fcitx.Controller1",
            "CurrentInputMethodGroup",
        ])
        .output();
    if let Ok(output) = out {
        if output.status.success() {
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            if let Some(start) = stdout_str.find('"') {
                if let Some(end) = stdout_str[start + 1..].find('"') {
                    let grp = &stdout_str[start + 1..start + 1 + end];
                    if !grp.is_empty() {
                        return grp.to_string();
                    }
                }
            }
        }
    }

    let grps_out = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.fcitx.Fcitx5",
            "/controller",
            "org.fcitx.Fcitx.Controller1",
            "InputMethodGroups",
        ])
        .output();
    if let Ok(output) = grps_out {
        if output.status.success() {
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            if let Some(start) = stdout_str.find('"') {
                if let Some(end) = stdout_str[start + 1..].find('"') {
                    let grp = &stdout_str[start + 1..start + 1 + end];
                    if !grp.is_empty() {
                        return grp.to_string();
                    }
                }
            }
        }
    }

    "Default".to_string()
}

// try adding clak and setting default im via fcitx5 dbus controller
fn setup_profile_via_dbus() -> bool {
    let active_group = get_active_im_group();
    let out = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.fcitx.Fcitx5",
            "/controller",
            "org.fcitx.Fcitx.Controller1",
            "InputMethodGroupInfo",
            "s",
            &active_group,
        ])
        .output();

    let Ok(output) = out else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut escaped = false;
    for ch in stdout_str.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            in_quote = !in_quote;
        } else if ch.is_whitespace() && !in_quote {
            if !cur.is_empty() {
                tokens.push(cur.clone());
                cur.clear();
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }

    if tokens.is_empty() || tokens[0] != "sa(ss)" {
        return false;
    }

    let raw_layout = tokens.get(1).map(|s| s.as_str()).unwrap_or("us");
    let layout = if raw_layout.is_empty() || raw_layout.starts_with("vn") {
        "us"
    } else {
        raw_layout
    };

    let mut items: Vec<(String, String)> = Vec::new();
    let mut i = 3;
    while i < tokens.len() {
        let name = tokens[i].clone();
        let sub = tokens.get(i + 1).cloned().unwrap_or_default();
        items.push((name, sub));
        i += 2;
    }

    if !items.iter().any(|(name, _)| name == "clak") {
        items.push(("clak".to_string(), "".to_string()));
    }

    let count_str = items.len().to_string();
    let mut call_args = vec![
        "--user".to_string(),
        "call".to_string(),
        "org.fcitx.Fcitx5".to_string(),
        "/controller".to_string(),
        "org.fcitx.Fcitx.Controller1".to_string(),
        "SetInputMethodGroupInfo".to_string(),
        "ssa(ss)".to_string(),
        active_group,
        layout.to_string(),
        count_str,
    ];
    for (name, sub) in &items {
        call_args.push(name.clone());
        call_args.push(sub.clone());
    }

    let set_res = Command::new("busctl").args(&call_args).status();
    if set_res.map(|s| s.success()).unwrap_or(false) {
        let _ = Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.fcitx.Fcitx5",
                "/controller",
                "org.fcitx.Fcitx.Controller1",
                "SetCurrentIM",
                "s",
                "clak",
            ])
            .status();
        let _ = Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.fcitx.Fcitx5",
                "/controller",
                "org.fcitx.Fcitx.Controller1",
                "Save",
            ])
            .status();
        return true;
    }

    false
}

// restart fcitx5 daemon cleanly and switch to clak
pub fn restart_fcitx5_daemon() -> bool {
    let initial_pids = super::daemon::check_daemon().fcitx5_pids;

    // try systemd user services if active
    let systemd_units = [
        "fcitx5.service",
        "app-org.fcitx.Fcitx5.service",
        "app-org.fcitx.Fcitx5@autostart.service",
    ];
    let mut restarted_via_systemd = false;
    for unit in systemd_units {
        let is_active = Command::new("systemctl")
            .args(["--user", "is-active", unit])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if is_active {
            let ok = Command::new("systemctl")
                .args(["--user", "restart", unit])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                restarted_via_systemd = true;
                break;
            }
        }
    }

    // use native replace flag if not managed by systemd
    if !restarted_via_systemd {
        let _ = Command::new("fcitx5")
            .args(["-r", "-d"])
            .status();
    }

    // poll for new pid or daemon readiness
    let mut success = false;
    for _ in 0..15 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        let daemon = super::daemon::check_daemon();
        if daemon.fcitx5_running && !daemon.fcitx5_pids.is_empty() {
            if initial_pids.is_empty() || daemon.fcitx5_pids != initial_pids {
                success = true;
                break;
            }
        }
    }

    // fallback kill and restart if previous instance refused to exit
    if !success && !initial_pids.is_empty() {
        for pid in &initial_pids {
            let _ = Command::new("kill").args(["-15", &pid.to_string()]).status();
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        let _ = Command::new("fcitx5").arg("-d").status();

        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(150));
            let daemon = super::daemon::check_daemon();
            if daemon.fcitx5_running && !daemon.fcitx5_pids.is_empty() {
                success = true;
                break;
            }
        }
    }

    // ensure clak is active input method
    if success {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let _ = Command::new("fcitx5-remote").args(["-s", "clak"]).status();
    }

    success
}

// automatically insert clak into fcitx5 profile, fix ubuntu paths, and reload daemon
pub fn fix_fcitx5_profile() -> Result<(), String> {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());

    // 1. Tao file bien moi truong 99-clak-im.conf neu chua co
    let env_dir = PathBuf::from(&home).join(".config/environment.d");
    let env_file = env_dir.join("99-clak-im.conf");
    let env_content =
        "QT_IM_MODULE=fcitx\nQT_IM_MODULES=\"wayland;fcitx\"\nXMODIFIERS=@im=fcitx\nINPUT_METHOD=fcitx5\nSDL_IM_MODULE=fcitx\n";
    if !env_file.exists() && fs::create_dir_all(&env_dir).is_ok() {
        let _ = fs::write(&env_file, env_content);
    }
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

    // 2. Kiem tra va dam bao duong dan addon tren Ubuntu
    // Fcitx5 tren Ubuntu multiarch tim lib trong x86_64-linux-gnu hoac absolute path
    let local_lib_dir = PathBuf::from(&home).join(".local/lib/fcitx5");
    let local_multiarch_dir = PathBuf::from(&home).join(".local/lib/x86_64-linux-gnu/fcitx5");
    let lib_so = local_lib_dir.join("libclak.so");
    if lib_so.exists() {
        let _ = fs::create_dir_all(&local_multiarch_dir);
        let multiarch_so = local_multiarch_dir.join("libclak.so");
        if !multiarch_so.exists() {
            #[cfg(unix)]
            let _ = std::os::unix::fs::symlink(&lib_so, &multiarch_so);
        }

        // Dam bao addon/clak.conf tro den duong dan tuyet doi neu la user install
        let user_addon_conf = PathBuf::from(&home).join(".local/share/fcitx5/addon/clak.conf");
        if user_addon_conf.exists() {
            if let Ok(c) = fs::read_to_string(&user_addon_conf) {
                let lib_path_str = lib_so.to_string_lossy();
                let base_lib = lib_path_str.trim_end_matches(".so");
                if c.contains("Library=libclak") || !c.contains(base_lib) {
                    let updated = c.replace("Library=libclak", &format!("Library={}", base_lib));
                    let _ = fs::write(&user_addon_conf, updated);
                }
            }
        }
    }

    // 3. Cap nhat profile qua DBus controller (nhanh, khong mat state)
    let _ = setup_profile_via_dbus();

    // 4. Luon dam bao file ~/.config/fcitx5/profile tren disk co clak va Default Layout=us
    let config_dir = PathBuf::from(&home).join(".config/fcitx5");
    let profile_path = config_dir.join("profile");
    let _ = fs::create_dir_all(&config_dir);

    if !profile_path.exists() {
        let default_content = r#"[Groups/0]
Name=Default
Default Layout=us
DefaultIM=clak

[Groups/0/Items/0]
Name=keyboard-us
Layout=

[Groups/0/Items/1]
Name=clak
Layout=

[GroupOrder]
0=Default
"#;
        let _ = fs::write(&profile_path, default_content);
    } else if let Ok(content) = fs::read_to_string(&profile_path) {
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let has_clak = lines.iter().any(|l| l.trim() == "Name=clak");

        if !has_clak {
            let item_count = lines
                .iter()
                .filter(|l| l.contains("[Groups/0/Items/"))
                .count();

            let insert_idx = lines
                .iter()
                .position(|l| l.trim() == "[GroupOrder]")
                .unwrap_or(lines.len());

            let mut block = vec![
                format!("[Groups/0/Items/{}]", item_count),
                "Name=clak".to_string(),
                "Layout=".to_string(),
                "".to_string(),
            ];

            for (i, bline) in block.drain(..).enumerate() {
                lines.insert(insert_idx + i, bline);
            }
        }

        let mut found_default_im = false;
        for line in &mut lines {
            if line.starts_with("DefaultIM=") {
                *line = "DefaultIM=clak".to_string();
                found_default_im = true;
            } else if line.starts_with("Default Layout=vn") {
                *line = "Default Layout=us".to_string();
            }
        }

        if !found_default_im {
            if let Some(pos) = lines.iter().position(|l| l.trim() == "[Groups/0]") {
                lines.insert(pos + 1, "DefaultIM=clak".to_string());
            }
        }

        let new_content = lines.join("\n") + "\n";
        let _ = fs::write(&profile_path, new_content);
    }

    // 5. Luon khoi dong lai fcitx5 daemon de dam bao SharedLibrary duoc dlopen
    restart_fcitx5_daemon();

    Ok(())
}

// inspect wps office installation and compatibility configuration
pub fn check_wps() -> WpsStatus {
    let has_bin = Path::new("/usr/bin/wps").exists();
    let has_office6 = Path::new("/usr/lib/office6").exists();
    let mut has_desktop = false;

    if let Ok(entries) = fs::read_dir("/usr/share/applications") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("wps-office-") && name_str.ends_with(".desktop") {
                has_desktop = true;
                break;
            }
        }
    }

    let installed = has_bin || has_office6 || has_desktop;
    if !installed {
        return WpsStatus {
            installed: false,
            configured: true,
            desktop_files_patched: 0,
            explanation: "Hệ thống không cài đặt WPS Office".to_string(),
        };
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let user_app_dir = PathBuf::from(&home).join(".local/share/applications");
    let mut desktop_files_patched = 0;

    if let Ok(entries) = fs::read_dir(&user_app_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("wps-office-") && name_str.ends_with(".desktop") {
                if let Ok(c) = fs::read_to_string(entry.path()) {
                    if c.contains("QT_IM_MODULE=fcitx") {
                        desktop_files_patched += 1;
                    }
                }
            }
        }
    }

    let env_file = PathBuf::from(&home).join(".config/environment.d/99-clak-wps.conf");
    let env_configured = env_file.exists();
    let configured = desktop_files_patched > 0 || env_configured;

    WpsStatus {
        installed: true,
        configured,
        desktop_files_patched,
        explanation: "WPS Office chạy Qt5 nội bộ trên XWayland. Cần uinput và QT_IM_MODULE=fcitx để không bị nuốt dấu và nhận bàn phím mượt mà.".to_string(),
    }
}

// fix wps office desktop launcher and environment variable
pub fn fix_wps_compatibility() -> Result<usize, String> {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let user_app_dir = PathBuf::from(&home).join(".local/share/applications");
    fs::create_dir_all(&user_app_dir)
        .map_err(|e| format!("không thể tạo thư mục applications: {}", e))?;

    let mut patched_count = 0;
    if let Ok(entries) = fs::read_dir("/usr/share/applications") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("wps-office-") && name_str.ends_with(".desktop") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let mut modified_lines = Vec::new();
                    for line in content.lines() {
                        if line.starts_with("Exec=/usr/bin/") {
                            modified_lines.push(line.replacen(
                                "Exec=/usr/bin/",
                                "Exec=env QT_IM_MODULE=fcitx /usr/bin/",
                                1,
                            ));
                        } else if line.starts_with("Exec=") && !line.contains("QT_IM_MODULE") {
                            modified_lines.push(line.replacen(
                                "Exec=",
                                "Exec=env QT_IM_MODULE=fcitx ",
                                1,
                            ));
                        } else {
                            modified_lines.push(line.to_string());
                        }
                    }
                    let out_path = user_app_dir.join(&*name_str);
                    if fs::write(&out_path, modified_lines.join("\n") + "\n").is_ok() {
                        patched_count += 1;
                    }
                }
            }
        }
    }

    // tao file environment.d
    let env_dir = PathBuf::from(&home).join(".config/environment.d");
    if fs::create_dir_all(&env_dir).is_ok() {
        let env_file = env_dir.join("99-clak-wps.conf");
        let _ = fs::write(env_file, "QT_IM_MODULE=fcitx\n");
    }

    // cap nhat desktop database
    let _ = Command::new("update-desktop-database")
        .arg(&user_app_dir)
        .status();

    Ok(patched_count)
}

// inspect jetbrains ides in user config
pub fn check_jetbrains() -> JetBrainsStatus {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let jb_dir = PathBuf::from(&home).join(".config/JetBrains");

    if !jb_dir.exists() {
        return JetBrainsStatus {
            installed: false,
            unconfigured_ides: Vec::new(),
            configured_ides: Vec::new(),
            explanation: "Hệ thống không tìm thấy thư mục cấu hình JetBrains".to_string(),
        };
    }

    let mut unconfigured_ides = Vec::new();
    let mut configured_ides = Vec::new();

    if let Ok(entries) = fs::read_dir(&jb_dir) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                let name = entry.file_name();
                let name_str = name.to_string_lossy().to_string();

                let is_ide = name_str.starts_with("IntelliJIdea")
                    || name_str.starts_with("Idea")
                    || name_str.starts_with("PyCharm")
                    || name_str.starts_with("CLion")
                    || name_str.starts_with("WebStorm")
                    || name_str.starts_with("Rider")
                    || name_str.starts_with("DataGrip")
                    || name_str.starts_with("RustRover")
                    || name_str.starts_with("GoLand")
                    || name_str.starts_with("PhpStorm")
                    || name_str.starts_with("RubyMine")
                    || name_str.starts_with("Aqua")
                    || name_str.starts_with("DataSpell")
                    || name_str.starts_with("Gateway")
                    || name_str.starts_with("AndroidStudio")
                    || name_str.ends_with("Studio");

                if !is_ide {
                    continue;
                }

                // tim file *64.vmoptions
                let mut is_configured = false;

                if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                    for sub in sub_entries.flatten() {
                        let sub_name = sub.file_name();
                        let sub_str = sub_name.to_string_lossy();
                        if sub_str.ends_with("64.vmoptions") {
                            if let Ok(vmo) = fs::read_to_string(sub.path()) {
                                if vmo.contains("sun.awt.X11.XToolkit") {
                                    is_configured = true;
                                    break;
                                }
                            }
                        }
                    }
                }

                if is_configured {
                    configured_ides.push(name_str);
                } else {
                    unconfigured_ides.push(name_str);
                }
            }
        }
    }

    let installed = !configured_ides.is_empty() || !unconfigured_ides.is_empty();

    JetBrainsStatus {
        installed,
        unconfigured_ides,
        configured_ides,
        explanation: "Java JBR trên Wayland có lỗi JBR-5672 gây mất focus bộ gõ khi đổi tab. Thêm XToolkit vào .vmoptions giúp bộ gõ nhận diện liên tục và không bị đơ.".to_string(),
    }
}

// automatically patch jetbrains vmoptions
pub fn fix_jetbrains_compatibility() -> Result<usize, String> {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let jb_dir = PathBuf::from(&home).join(".config/JetBrains");

    if !jb_dir.exists() {
        return Ok(0);
    }

    let mut patched_count = 0;

    if let Ok(entries) = fs::read_dir(&jb_dir) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                let name = entry.file_name();
                let dir_name = name.to_string_lossy().to_string();

                let prefix = if dir_name.starts_with("IntelliJ") || dir_name.starts_with("Idea") {
                    "idea"
                } else if dir_name.starts_with("PyCharm") {
                    "pycharm"
                } else if dir_name.starts_with("CLion") {
                    "clion"
                } else if dir_name.starts_with("WebStorm") {
                    "webstorm"
                } else if dir_name.starts_with("Rider") {
                    "rider"
                } else if dir_name.starts_with("DataGrip") {
                    "datagrip"
                } else if dir_name.starts_with("RustRover") {
                    "rustrover"
                } else if dir_name.starts_with("GoLand") {
                    "goland"
                } else if dir_name.starts_with("PhpStorm") {
                    "phpstorm"
                } else if dir_name.starts_with("RubyMine") {
                    "rubymine"
                } else if dir_name.starts_with("Aqua") {
                    "aqua"
                } else if dir_name.starts_with("DataSpell") {
                    "dataspell"
                } else if dir_name.starts_with("Gateway") {
                    "gateway"
                } else if dir_name.contains("Studio") {
                    "studio"
                } else {
                    continue;
                };

                let target_vmo = entry.path().join(format!("{}64.vmoptions", prefix));

                let option_lines = "\n-Dawt.toolkit.name=sun.awt.X11.XToolkit\n-Didea.input.method.disabler.notification.muted=true\n";

                if target_vmo.exists() {
                    let mut content = fs::read_to_string(&target_vmo).unwrap_or_default();
                    let mut needs_append = false;
                    if !content.contains("sun.awt.X11.XToolkit") {
                        content.push_str("\n-Dawt.toolkit.name=sun.awt.X11.XToolkit\n");
                        needs_append = true;
                    }
                    if !content.contains("idea.input.method.disabler.notification.muted=true") {
                        content.push_str("-Didea.input.method.disabler.notification.muted=true\n");
                        needs_append = true;
                    }
                    if needs_append && fs::write(&target_vmo, content).is_ok() {
                        patched_count += 1;
                    }
                } else if fs::write(&target_vmo, option_lines).is_ok() {
                    patched_count += 1;
                }
            }
        }
    }

    Ok(patched_count)
}

// inspect kde plasma virtual keyboard settings
pub fn check_kde() -> KdeStatus {
    let de = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let kde_full = env::var("KDE_FULL_SESSION").unwrap_or_default();
    let is_kde =
        de.contains("KDE") || de.contains("Plasma") || de.contains("plasma") || kde_full == "true";

    if !is_kde {
        return KdeStatus {
            is_kde: false,
            virtual_keyboard_configured: false,
            input_method_entry: None,
            explanation: "Không phát hiện môi trường KDE Plasma".to_string(),
        };
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let kwinrc_path = PathBuf::from(&home).join(".config/kwinrc");

    let mut virtual_keyboard_configured = false;
    let mut input_method_entry = None;

    if kwinrc_path.exists() {
        if let Ok(content) = fs::read_to_string(&kwinrc_path) {
            let mut in_wayland = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    in_wayland = trimmed == "[Wayland]";
                    continue;
                }
                if in_wayland && trimmed.starts_with("InputMethod=") {
                    let val = trimmed.trim_start_matches("InputMethod=").trim();
                    input_method_entry = Some(val.to_string());
                    if val.contains("Fcitx5") || val.contains("fcitx") {
                        virtual_keyboard_configured = true;
                    }
                }
            }
        }
    }

    // fallback check with kreadconfig6 or kreadconfig5 if file did not explicitly match
    if !virtual_keyboard_configured {
        for cmd in &["kreadconfig6", "kreadconfig5"] {
            if let Ok(out) = Command::new(cmd)
                .args([
                    "--file",
                    "kwinrc",
                    "--group",
                    "Wayland",
                    "--key",
                    "InputMethod",
                ])
                .output()
            {
                if out.status.success() {
                    let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !val.is_empty() {
                        input_method_entry = Some(val.clone());
                        if val.contains("Fcitx5") || val.contains("fcitx") {
                            virtual_keyboard_configured = true;
                            break;
                        }
                    }
                }
            }
        }
    }

    let explanation = if virtual_keyboard_configured {
        "KDE Plasma đã cấu hình VirtualKeyboard trỏ về Fcitx5 thành công".to_string()
    } else {
        "KWin Wayland cần đặt VirtualKeyboard thành Fcitx5 để bộ gõ hiển thị khung popup và hoạt động chính xác".to_string()
    };

    KdeStatus {
        is_kde: true,
        virtual_keyboard_configured,
        input_method_entry,
        explanation,
    }
}

// configure kwinrc virtual keyboard for fcitx5 on kde plasma
pub fn fix_kde_virtual_keyboard() -> Result<(), String> {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let kwinrc_path = PathBuf::from(&home).join(".config/kwinrc");
    let launcher_desktop = "/usr/share/applications/fcitx5-wayland-launcher.desktop";
    let target_desktop = if Path::new(launcher_desktop).exists() {
        launcher_desktop
    } else {
        "/usr/share/applications/org.fcitx.Fcitx5.desktop"
    };

    let mut kwrite_ok = false;
    for cmd in &["kwriteconfig6", "kwriteconfig5"] {
        let im_ok = Command::new(cmd)
            .args([
                "--file",
                "kwinrc",
                "--group",
                "Wayland",
                "--key",
                "InputMethod",
                target_desktop,
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let vk_ok = Command::new(cmd)
            .args([
                "--file",
                "kwinrc",
                "--group",
                "Wayland",
                "--key",
                "VirtualKeyboardEnabled",
                "true",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if im_ok && vk_ok {
            kwrite_ok = true;
            break;
        }
    }

    if !kwrite_ok {
        // fallback manual file edit
        let mut lines = Vec::new();
        let mut in_wayland = false;
        let mut found_wayland = false;
        let mut updated_im = false;
        let mut updated_vk = false;

        if kwinrc_path.exists() {
            if let Ok(content) = fs::read_to_string(&kwinrc_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('[') && trimmed.ends_with(']') {
                        if in_wayland {
                            if !updated_im {
                                lines.push(format!("InputMethod={}", target_desktop));
                                updated_im = true;
                            }
                            if !updated_vk {
                                lines.push("VirtualKeyboardEnabled=true".to_string());
                                updated_vk = true;
                            }
                        }
                        in_wayland = trimmed == "[Wayland]";
                        if in_wayland {
                            found_wayland = true;
                        }
                        lines.push(line.to_string());
                        continue;
                    }
                    if in_wayland && trimmed.starts_with("InputMethod=") {
                        lines.push(format!("InputMethod={}", target_desktop));
                        updated_im = true;
                        continue;
                    }
                    if in_wayland && trimmed.starts_with("VirtualKeyboardEnabled=") {
                        lines.push("VirtualKeyboardEnabled=true".to_string());
                        updated_vk = true;
                        continue;
                    }
                    lines.push(line.to_string());
                }
                if in_wayland {
                    if !updated_im {
                        lines.push(format!("InputMethod={}", target_desktop));
                    }
                    if !updated_vk {
                        lines.push("VirtualKeyboardEnabled=true".to_string());
                    }
                }
            }
        }

        if !found_wayland {
            lines.push("[Wayland]".to_string());
            lines.push(format!("InputMethod={}", target_desktop));
            lines.push("VirtualKeyboardEnabled=true".to_string());
        }

        fs::write(&kwinrc_path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    }

    // reload kwin virtual keyboard without requiring session relogin
    for prop in &["active", "enabled"] {
        let _ = Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.kde.KWin",
                "/VirtualKeyboard",
                "org.freedesktop.DBus.Properties",
                "Set",
                "ssv",
                "org.kde.kwin.VirtualKeyboard",
                prop,
                "b",
                "false",
            ])
            .status();
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    for prop in &["active", "enabled"] {
        let _ = Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.kde.KWin",
                "/VirtualKeyboard",
                "org.freedesktop.DBus.Properties",
                "Set",
                "ssv",
                "org.kde.kwin.VirtualKeyboard",
                prop,
                "b",
                "true",
            ])
            .status();
    }
    let _ = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.kde.KWin",
            "/KWin",
            "org.kde.KWin",
            "reconfigure",
        ])
        .status();
    for qdbus_bin in &["qdbus6", "qdbus"] {
        let _ = Command::new(qdbus_bin)
            .args([
                "org.kde.KWin",
                "/VirtualKeyboard",
                "org.freedesktop.DBus.Properties.Set",
                "org.kde.kwin.VirtualKeyboard",
                "enabled",
                "false",
            ])
            .status();
        let _ = Command::new(qdbus_bin)
            .args([
                "org.kde.KWin",
                "/VirtualKeyboard",
                "org.freedesktop.DBus.Properties.Set",
                "org.kde.kwin.VirtualKeyboard",
                "enabled",
                "true",
            ])
            .status();
    }

    // restart fcitx5 daemon to re-bind to kwin wayland input method protocol
    restart_fcitx5_daemon();

    Ok(())
}
