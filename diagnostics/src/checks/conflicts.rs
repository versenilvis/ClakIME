use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConflict {
    pub app_name: String,
    pub pid: u32,
    pub description: String,
    pub resolution_hint: String,
}

// scan running processes for conflicting input daemons and apps
pub fn check_conflicts() -> Vec<AppConflict> {
    let mut conflicts = Vec::new();

    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if let Ok(pid) = name_str.parse::<u32>() {
                let cmdline_path = entry.path().join("cmdline");
                let comm_path = entry.path().join("comm");

                let comm = fs::read_to_string(&comm_path)
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                let cmd_bytes = fs::read(&cmdline_path).unwrap_or_default();
                let cmd_str = String::from_utf8_lossy(&cmd_bytes);

                // check ibus daemon conflict
                if comm == "ibus-daemon" || cmd_str.contains("ibus-daemon") {
                    conflicts.push(AppConflict {
                        app_name: "IBus Daemon".to_string(),
                        pid,
                        description: format!(
                            "Tiến trình IBus daemon (PID {}) đang chạy ngầm, gây tranh chấp bắt phím và nuốt chữ với Fcitx5",
                            pid
                        ),
                        resolution_hint: "Tắt IBus bằng lệnh: ibus exit || systemctl --user stop ibus.service".to_string(),
                    });
                    continue;
                }

                // check ibus bamboo engine conflict
                if comm.contains("ibus-bamboo") || cmd_str.contains("ibus-engine-bamboo") {
                    conflicts.push(AppConflict {
                        app_name: "IBus Bamboo".to_string(),
                        pid,
                        description: format!(
                            "Bộ gõ Bamboo cho IBus (PID {}) đang chạy ngầm gây xung đột nhập liệu",
                            pid
                        ),
                        resolution_hint: "Tắt tiến trình bằng lệnh: killall ibus-engine-bamboo".to_string(),
                    });
                    continue;
                }

                // check legacy fcitx 4 daemon
                if (comm == "fcitx" || cmd_str.starts_with("fcitx\0")) && !cmd_str.contains("fcitx5") {
                    conflicts.push(AppConflict {
                        app_name: "Fcitx 4".to_string(),
                        pid,
                        description: format!(
                            "Tiến trình Fcitx 4 cũ (PID {}) đang chạy song song với Fcitx 5",
                            pid
                        ),
                        resolution_hint: "Tắt Fcitx 4 bằng lệnh: killall fcitx".to_string(),
                    });
                    continue;
                }

                // check wps office compose plugin fallback
                if cmd_str.contains("office6") || cmd_str.contains("/wps") {
                    let maps_path = format!("/proc/{}/maps", pid);
                    if let Ok(maps_content) = fs::read_to_string(&maps_path) {
                        let has_fcitx_plugin =
                            maps_content.contains("libfcitxplatforminputcontextplugin.so");
                        let has_compose_plugin =
                            maps_content.contains("libcomposeplatforminputcontextplugin.so");

                        if !has_fcitx_plugin && has_compose_plugin {
                            conflicts.push(AppConflict {
                                app_name: "WPS Office".to_string(),
                                pid,
                                description: format!(
                                    "WPS Office (PID {}) đang chạy với compose plugin do thiếu QT_IM_MODULE=fcitx",
                                    pid
                                ),
                                resolution_hint: "Đóng WPS và khởi chạy lại bằng lệnh: QT_IM_MODULE=fcitx wps".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    conflicts
}
