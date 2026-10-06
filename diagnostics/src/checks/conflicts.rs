use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConflict {
    pub app_name: String,
    pub pid: u32,
    pub description: String,
    pub resolution_hint: String,
}

pub fn check_conflicts() -> Vec<AppConflict> {
    let mut conflicts = Vec::new();

    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if let Ok(pid) = name_str.parse::<u32>() {
                let cmdline_path = entry.path().join("cmdline");
                if let Ok(cmd_bytes) = fs::read(&cmdline_path) {
                    let cmd_str = String::from_utf8_lossy(&cmd_bytes);
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
    }

    conflicts
}
