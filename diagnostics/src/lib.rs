pub mod checks;
pub mod report;

use serde::{Deserialize, Serialize};

pub use checks::autostart::{check_autostart, AutostartStatus};
pub use checks::conflicts::{check_conflicts, AppConflict};
pub use checks::daemon::{check_daemon, DaemonStatus};
pub use checks::environment::{check_environment, EnvironmentStatus};
pub use checks::frontends::{check_frontends, FrontendStatus};
pub use checks::memory::{check_memory, MemoryInspection};
pub use checks::permissions::{check_permissions, PermissionChecks};
pub use checks::system::{check_system, SystemInfo};
pub use checks::config::{check_config, ConfigStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticIssue {
    pub title: String,
    pub message: String,
    pub fix_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticWarning {
    pub title: String,
    pub message: String,
    pub fix_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub system: SystemInfo,
    pub permissions: PermissionChecks,
    pub daemon: DaemonStatus,
    pub memory: MemoryInspection,
    pub config: ConfigStatus,
    pub autostart: AutostartStatus,
    pub environment: EnvironmentStatus,
    pub frontends: FrontendStatus,
    pub conflicts: Vec<AppConflict>,
    pub issues: Vec<DiagnosticIssue>,
    pub warnings: Vec<DiagnosticWarning>,
}

impl DiagnosticReport {
    pub fn to_markdown(&self) -> String {
        report::format_markdown(self)
    }

    pub fn is_healthy(&self) -> bool {
        self.issues.is_empty()
    }
}

// purely read-only diagnostics execution with zero side-effects
pub fn run_diagnostics() -> DiagnosticReport {
    let system = check_system();
    let permissions = check_permissions();
    let daemon = check_daemon();
    let memory = check_memory(&daemon.fcitx5_pids);
    let config = check_config();
    let autostart = check_autostart();
    let environment = check_environment();
    let frontends = check_frontends();
    let conflicts = check_conflicts();

    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    if !permissions.uinput_writable {
        issues.push(DiagnosticIssue {
            title: "Thiếu quyền /dev/uinput".to_string(),
            message: "Người dùng chưa có quyền ghi vào /dev/uinput để gửi phím ảo khi cần".to_string(),
            fix_command: Some("echo 'KERNEL==\"uinput\", SUBSYSTEM==\"misc\", TAG+=\"uaccess\"' | sudo tee /etc/udev/rules.d/99-uinput.rules && sudo udevadm trigger /dev/uinput".to_string()),
        });
    }

    if permissions.input_total_count > 0 && permissions.input_readable_count == 0 {
        warnings.push(DiagnosticWarning {
            title: "Không đọc được /dev/input".to_string(),
            message: "Thiếu quyền đọc chuột để tự động reset buffer khi click chuột".to_string(),
            fix_command: Some("sudo usermod -aG input $USER".to_string()),
        });
    }

    if !daemon.fcitx5_running {
        issues.push(DiagnosticIssue {
            title: "Fcitx5 chưa chạy".to_string(),
            message: "Tiến trình Fcitx5 daemon chưa khởi chạy trong session hiện tại".to_string(),
            fix_command: Some("fcitx5 -d".to_string()),
        });
    } else {
        if let Some(ref im) = daemon.current_im {
            if im != "clak" {
                warnings.push(DiagnosticWarning {
                    title: "Chưa chọn bộ gõ Clak".to_string(),
                    message: format!("Bộ gõ đang kích hoạt là '{}', chưa chuyển sang Clak", im),
                    fix_command: Some("fcitx5-remote -s clak".to_string()),
                });
            }
        }

        if !memory.clak_loaded {
            issues.push(DiagnosticIssue {
                title: "Chưa nạp libclak.so".to_string(),
                message: "Fcitx5 chưa nạp thư viện libclak.so vào tiến trình".to_string(),
                fix_command: Some("Đảm bảo addon đã được cài vào ~/.local/lib/fcitx5 hoặc /usr/lib/fcitx5 và khởi động lại: fcitx5 -r -d".to_string()),
            });
        } else if memory.is_deleted_inode {
            warnings.push(DiagnosticWarning {
                title: "Thư viện cũ trong RAM".to_string(),
                message: "Fcitx5 đang giữ inode cũ của libclak.so trong RAM sau khi build/cài đè file mới".to_string(),
                fix_command: Some("fcitx5 -r -d".to_string()),
            });
        }
    }

    if !autostart.is_enabled {
        warnings.push(DiagnosticWarning {
            title: "Chưa bật khởi động cùng hệ thống".to_string(),
            message: "Chưa kích hoạt tự động chạy Clak khi đăng nhập vào hệ thống".to_string(),
            fix_command: Some("clak autostart --enable".to_string()),
        });
    }

    // validate environment variables
    if let Some(ref gtk_im) = environment.gtk_im_module {
        if gtk_im == "ibus" {
            issues.push(DiagnosticIssue {
                title: "GTK_IM_MODULE xung đột với IBus".to_string(),
                message: "GTK_IM_MODULE đang được đặt là 'ibus', khiến ứng dụng GTK bỏ qua Fcitx5 và gây nuốt chữ".to_string(),
                fix_command: Some("Xóa hoặc đổi GTK_IM_MODULE=fcitx trong ~/.config/environment.d/99-clak-im.conf hoặc ~/.profile".to_string()),
            });
        }
    }
    if let Some(ref qt_im) = environment.qt_im_module {
        if qt_im == "ibus" {
            issues.push(DiagnosticIssue {
                title: "QT_IM_MODULE xung đột với IBus".to_string(),
                message: "QT_IM_MODULE đang được đặt là 'ibus', khiến ứng dụng Qt bỏ qua Fcitx5".to_string(),
                fix_command: Some("Đổi QT_IM_MODULE=fcitx trong ~/.config/environment.d/99-clak-im.conf".to_string()),
            });
        }
    }
    if let Some(ref xmod) = environment.xmodifiers {
        if xmod.contains("ibus") {
            warnings.push(DiagnosticWarning {
                title: "XMODIFIERS xung đột với IBus".to_string(),
                message: format!("XMODIFIERS đang là '{}', cần đổi thành '@im=fcitx'", xmod),
                fix_command: Some("export XMODIFIERS=@im=fcitx".to_string()),
            });
        }
    }

    if !environment.env_file_exists {
        warnings.push(DiagnosticWarning {
            title: "Chưa cấu hình 99-clak-im.conf".to_string(),
            message: "Chưa tìm thấy file ~/.config/environment.d/99-clak-im.conf được sinh bởi trình cài đặt".to_string(),
            fix_command: Some("Chạy bash scripts/install.sh để cấu hình biến môi trường tự động".to_string()),
        });
    }

    // check frontend packages
    if !frontends.gtk3 && !frontends.gtk4 {
        warnings.push(DiagnosticWarning {
            title: "Thiếu module Fcitx5 Frontend GTK".to_string(),
            message: "Chưa tìm thấy thư viện im-fcitx5 cho GTK3/GTK4. Các ứng dụng GTK có thể fallback về XIM và nuốt chữ khi gõ nhanh".to_string(),
            fix_command: Some("Cài đặt frontend GTK: Ubuntu/Debian: sudo apt install fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 | Arch: sudo pacman -S fcitx5-gtk".to_string()),
        });
    }
    if !frontends.qt5 && !frontends.qt6 {
        warnings.push(DiagnosticWarning {
            title: "Thiếu module Fcitx5 Frontend Qt".to_string(),
            message: "Chưa tìm thấy plugin fcitx5platforminputcontext cho Qt5/Qt6".to_string(),
            fix_command: Some("Cài đặt frontend Qt: Ubuntu/Debian: sudo apt install fcitx5-frontend-qt5 fcitx5-frontend-qt6 | Arch: sudo pacman -S fcitx5-qt".to_string()),
        });
    }

    for conf in &conflicts {
        warnings.push(DiagnosticWarning {
            title: format!("Xung đột ứng dụng: {}", conf.app_name),
            message: conf.description.clone(),
            fix_command: Some(conf.resolution_hint.clone()),
        });
    }

    DiagnosticReport {
        system,
        permissions,
        daemon,
        memory,
        config,
        autostart,
        environment,
        frontends,
        conflicts,
        issues,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostics_runs_cleanly_without_panics() {
        let report = run_diagnostics();
        assert!(!report.system.architecture.is_empty());
        let md = report.to_markdown();
        assert!(md.contains("# Clak Doctor: Báo Cáo Chẩn Đoán Hệ Thống"));
    }

    #[test]
    fn test_report_serialization() {
        let report = run_diagnostics();
        let json = serde_json::to_string(&report).expect("failed to serialize report");
        assert!(json.contains("system"));
        assert!(json.contains("permissions"));
        assert!(json.contains("environment"));
        assert!(json.contains("frontends"));
    }
}
