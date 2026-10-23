pub mod checks;
pub mod report;

use serde::{Deserialize, Serialize};

pub use checks::autostart::{
    check_autostart, disable_autostart, enable_autostart, AutostartStatus,
};
pub use checks::compatibility::{
    check_jetbrains, check_kde, check_profile, check_wps, fix_fcitx5_profile,
    fix_jetbrains_compatibility, fix_kde_virtual_keyboard, fix_wps_compatibility,
    restart_fcitx5_daemon, JetBrainsStatus, KdeStatus, ProfileStatus, WpsStatus,
};
pub use checks::config::{check_config, ConfigStatus};
pub use checks::conflicts::{check_conflicts, AppConflict};
pub use checks::daemon::{check_daemon, DaemonStatus};
pub use checks::display::{check_display, DisplayEnvironment, MonitorInfo};
pub use checks::environment::{check_environment, EnvironmentStatus};
pub use checks::frontends::{check_frontends, FrontendStatus};
pub use checks::latency::{check_latency, LatencyBenchmark};
pub use checks::memory::{check_memory, MemoryInspection};
pub use checks::pacing_calc::{calculate_pacing, PacingRecommendation};
pub use checks::permissions::{check_permissions, fix_input_permissions, PermissionChecks};
pub use checks::system::{check_system, SystemInfo};

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
    pub profile: ProfileStatus,
    pub wps: WpsStatus,
    pub jetbrains: JetBrainsStatus,
    pub kde: KdeStatus,
    pub display: DisplayEnvironment,
    pub latency: LatencyBenchmark,
    pub pacing: PacingRecommendation,
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
    let profile = check_profile();
    let wps = check_wps();
    let jetbrains = check_jetbrains();
    let kde = check_kde();

    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    if !permissions.uinput_writable {
        if permissions.xtest_available {
            warnings.push(DiagnosticWarning {
                title: "Đang dùng XTest fallback cho phím ảo".to_string(),
                message: "Thiếu quyền ghi /dev/uinput, đang tạm dùng XTest trên X11/XWayland. Khuyên dùng uinput để hỗ trợ toàn diện trên Wayland và terminal".to_string(),
                fix_command: Some("clak doctor --fix".to_string()),
            });
        } else {
            issues.push(DiagnosticIssue {
                title: "Thiếu quyền /dev/uinput".to_string(),
                message: "Người dùng chưa có quyền ghi vào /dev/uinput để gửi phím ảo khi cần"
                    .to_string(),
                fix_command: Some("clak doctor --fix".to_string()),
            });
        }
    }

    if permissions.input_total_count > 0 && permissions.input_readable_count == 0 {
        warnings.push(DiagnosticWarning {
            title: "Không đọc được /dev/input".to_string(),
            message: "Thiếu quyền đọc chuột để tự động reset buffer khi click chuột".to_string(),
            fix_command: Some("clak doctor --fix".to_string()),
        });
    }

    if !profile.clak_in_profile {
        issues.push(DiagnosticIssue {
            title: "Chưa thêm Clak vào Fcitx5".to_string(),
            message: "Clak chưa được thêm vào danh sách bộ gõ trong ~/.config/fcitx5/profile"
                .to_string(),
            fix_command: Some(
                "Bấm nút 'Kích hoạt 1-Click' trên thanh thông báo hoặc chạy clak doctor"
                    .to_string(),
            ),
        });
    } else if !profile.is_default_im {
        warnings.push(DiagnosticWarning {
            title: "Chưa đặt Clak làm mặc định".to_string(),
            message: "Clak đã có trong danh sách nhưng DefaultIM chưa phải là clak".to_string(),
            fix_command: Some("Đặt DefaultIM=clak trong ~/.config/fcitx5/profile".to_string()),
        });
    }

    if wps.installed && !wps.configured {
        warnings.push(DiagnosticWarning {
            title: "WPS Office chưa tối ưu".to_string(),
            message: "WPS Office chạy Qt5 nội bộ trên XWayland cần biến QT_IM_MODULE=fcitx để không bị nuốt dấu".to_string(),
            fix_command: Some("Bấm nút 'Tối ưu hóa WPS Office' trong Tab Bác sĩ".to_string()),
        });
    }

    if jetbrains.installed && !jetbrains.unconfigured_ides.is_empty() {
        warnings.push(DiagnosticWarning {
            title: format!("JetBrains IDEs chưa tối ưu ({})", jetbrains.unconfigured_ides.join(", ")),
            message: "Java runtime trên Wayland có lỗi JBR-5672 gây mất focus khi đổi tab. Cần thêm -Dawt.toolkit.name=sun.awt.X11.XToolkit".to_string(),
            fix_command: Some("Bấm nút 'Tối ưu hóa JetBrains' trong Tab Bác sĩ".to_string()),
        });
    }

    if kde.is_kde && !kde.virtual_keyboard_configured {
        warnings.push(DiagnosticWarning {
            title: "KDE Plasma chưa đặt VirtualKeyboard cho Fcitx5".to_string(),
            message: "KWin Wayland cần đặt VirtualKeyboard thành Fcitx5 để bộ gõ hiển thị khung popup và hoạt động chính xác".to_string(),
            fix_command: Some("kwriteconfig6 --file kwinrc --group Wayland --key InputMethod /usr/share/applications/org.fcitx.Fcitx5.desktop hoặc chạy clak doctor --fix".to_string()),
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
                fix_command: Some("Xóa dòng GTK_IM_MODULE trong ~/.config/environment.d/99-clak-im.conf hoặc ~/.profile".to_string()),
            });
        }
    }
    if let Some(ref qt_im) = environment.qt_im_module {
        if qt_im == "ibus" {
            let (msg, fix) = if environment.env_file_exists {
                (
                    "QT_IM_MODULE trong phiên hiện tại đang nhận từ IBus. File 99-clak-im.conf đã có sẵn, cần đăng xuất và đăng nhập lại để GNOME nạp mới".to_string(),
                    "Đăng xuất và đăng nhập lại phiên làm việc (hoặc khởi động lại máy)".to_string(),
                )
            } else {
                (
                    "QT_IM_MODULE đang được đặt là 'ibus', khiến ứng dụng Qt bỏ qua Fcitx5"
                        .to_string(),
                    "Đổi QT_IM_MODULE=fcitx trong ~/.config/environment.d/99-clak-im.conf"
                        .to_string(),
                )
            };
            issues.push(DiagnosticIssue {
                title: "QT_IM_MODULE xung đột với IBus".to_string(),
                message: msg,
                fix_command: Some(fix),
            });
        }
    }
    if let Some(ref xmod) = environment.xmodifiers {
        if xmod.contains("ibus") {
            let fix = if environment.env_file_exists {
                "Đăng xuất và đăng nhập lại phiên làm việc (hoặc khởi động lại máy)".to_string()
            } else {
                "export XMODIFIERS=@im=fcitx".to_string()
            };
            warnings.push(DiagnosticWarning {
                title: "XMODIFIERS xung đột với IBus".to_string(),
                message: format!("XMODIFIERS đang là '{}', cần đổi thành '@im=fcitx'", xmod),
                fix_command: Some(fix),
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

    let display = check_display();
    let latency = check_latency();
    let pacing = calculate_pacing(&display, &latency);

    if display.is_mixed_refresh_rate {
        warnings.push(DiagnosticWarning {
            title: "Màn hình hỗn hợp (Mixed Refresh Rate)".to_string(),
            message: format!(
                "Phát hiện nhiều màn hình có tần số quét khác nhau. Khuyến nghị áp dụng mốc {:.0}Hz an toàn để chống nuốt chữ",
                display.effective_refresh_rate
            ),
            fix_command: None,
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
        profile,
        wps,
        jetbrains,
        kde,
        display,
        latency,
        pacing,
        conflicts,
        issues,
        warnings,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutoFixSummary {
    pub profile_fixed: bool,
    pub fcitx5_restarted: bool,
    pub autostart_enabled: bool,
    pub wps_patched: usize,
    pub jetbrains_patched: usize,
    pub kde_fixed: bool,
    pub permissions_fixed: bool,
    pub pacing_fixed: bool,
    pub errors: Vec<String>,
}

impl AutoFixSummary {
    pub fn has_any_action(&self) -> bool {
        self.profile_fixed
            || self.fcitx5_restarted
            || self.autostart_enabled
            || self.wps_patched > 0
            || self.jetbrains_patched > 0
            || self.kde_fixed
            || self.permissions_fixed
            || self.pacing_fixed
    }
}

// configure custom pacing profile from measured benchmark
pub fn apply_pacing_profile(pacing: &checks::pacing_calc::PacingRecommendation) -> Result<(), std::io::Error> {
    let mut cfg = clak_engine::config::ClakConfig::load();
    cfg.pacing.preset = "custom".to_string();
    cfg.pacing.draftjs_delay_ms = pacing.draftjs_post_delay_ms;
    cfg.pacing.draftjs_gap_ms = pacing.gap_ms;
    cfg.pacing.terminal_delay_ms = pacing.terminal_post_delay_ms;
    cfg.pacing.terminal_gap_ms = (pacing.gap_ms / 2).max(2);
    cfg.pacing.heavy_app_delay_ms = pacing.heavy_app_post_delay_ms;
    cfg.pacing.heavy_app_gap_ms = (pacing.gap_ms * 2 / 3).max(3);
    cfg.save()
}

pub fn auto_fix_all(include_permissions: bool) -> AutoFixSummary {
    let mut summary = AutoFixSummary::default();
    let pre_report = run_diagnostics();

    // 1. Setup profile if not configured
    if !pre_report.profile.clak_in_profile || !pre_report.profile.is_default_im {
        match fix_fcitx5_profile() {
            Ok(_) => summary.profile_fixed = true,
            Err(e) => summary.errors.push(format!("Lỗi cấu hình profile: {}", e)),
        }
    }

    // 2. Restart daemon if addon not loaded or deleted inode
    if pre_report.daemon.fcitx5_running
        && (!pre_report.memory.clak_loaded || pre_report.memory.is_deleted_inode)
    {
        if restart_fcitx5_daemon() {
            summary.fcitx5_restarted = true;
        } else {
            summary
                .errors
                .push("Khởi động lại Fcitx5 không thành công".to_string());
        }
    } else if pre_report.daemon.fcitx5_running
        && pre_report.daemon.current_im.as_deref() != Some("clak")
    {
        let _ = std::process::Command::new("fcitx5-remote")
            .args(["-s", "clak"])
            .status();
    }

    // 3. Enable autostart and configure environment variables
    let has_env_conflict = pre_report.environment.qt_im_module.as_deref() == Some("ibus")
        || pre_report
            .environment
            .xmodifiers
            .as_deref()
            .is_some_and(|x| x.contains("ibus"))
        || !pre_report.environment.env_file_exists;

    if !pre_report.autostart.is_enabled || has_env_conflict {
        match enable_autostart() {
            Ok(_) => summary.autostart_enabled = true,
            Err(e) => summary
                .errors
                .push(format!("Lỗi cấu hình khởi động và môi trường: {}", e)),
        }
    }

    // 4. Fix WPS compatibility if needed
    if pre_report.wps.installed && !pre_report.wps.configured {
        match fix_wps_compatibility() {
            Ok(count) => summary.wps_patched = count,
            Err(e) => summary.errors.push(format!("Lỗi cấu hình WPS: {}", e)),
        }
    }

    // 5. Fix JetBrains compatibility if needed
    if pre_report.jetbrains.installed && !pre_report.jetbrains.unconfigured_ides.is_empty() {
        match fix_jetbrains_compatibility() {
            Ok(count) => summary.jetbrains_patched = count,
            Err(e) => summary
                .errors
                .push(format!("Lỗi cấu hình JetBrains: {}", e)),
        }
    }

    // 5b. Fix KDE VirtualKeyboard if needed
    if pre_report.kde.is_kde && !pre_report.kde.virtual_keyboard_configured {
        match fix_kde_virtual_keyboard() {
            Ok(_) => {
                summary.kde_fixed = true;
                summary.fcitx5_restarted = true;
            }
            Err(e) => summary
                .errors
                .push(format!("Lỗi cấu hình KDE VirtualKeyboard: {}", e)),
        }
    }

    // 6. Fix permissions if requested and needed
    if include_permissions
        && (!pre_report.permissions.uinput_writable
            || pre_report.permissions.input_readable_count == 0)
    {
        match fix_input_permissions() {
            Ok(_) => summary.permissions_fixed = true,
            Err(e) => summary
                .errors
                .push(format!("Lỗi cấp quyền thiết bị: {}", e)),
        }
    }

    // 7. Auto-configure hardware-specific pacing profile
    match apply_pacing_profile(&pre_report.pacing) {
        Ok(_) => summary.pacing_fixed = true,
        Err(e) => summary
            .errors
            .push(format!("Lỗi lưu cấu hình pacing: {}", e)),
    }

    summary
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
        assert!(md.contains("## 9. Màn Hình & VSync Budget"));
        assert!(md.contains("## 10. Độ Trễ Phần Cứng & Pacing Đề Xuất"));
    }

    #[test]
    fn test_report_serialization() {
        let report = run_diagnostics();
        let json = serde_json::to_string(&report).expect("failed to serialize report");
        assert!(json.contains("system"));
        assert!(json.contains("permissions"));
        assert!(json.contains("environment"));
        assert!(json.contains("frontends"));
        assert!(json.contains("display"));
        assert!(json.contains("latency"));
        assert!(json.contains("pacing"));
    }

    #[test]
    fn test_pacing_calculation_formula() {
        let display = DisplayEnvironment {
            monitors: vec![MonitorInfo {
                name: "eDP-1".to_string(),
                width: 2560,
                height: 1600,
                refresh_rate: 120.0,
                is_active_focus: true,
                is_primary: true,
            }],
            effective_refresh_rate: 120.0,
            frame_budget_ms: 8.33,
            is_mixed_refresh_rate: false,
            detection_method: "Test".to_string(),
        };

        let latency = LatencyBenchmark {
            uinput_accessible: true,
            uinput_write_us: 10.0,
            measured_rtt_ms: 2.0,
            measured_jitter_ms: 0.5,
            keyboard_repeat_delay_ms: 500,
            keyboard_repeat_interval_ms: 30,
            scheduler_preempt: "PREEMPT_DYNAMIC".to_string(),
            system_load: 0.8,
        };

        let pacing = calculate_pacing(&display, &latency);
        assert!(pacing.gap_ms >= 2);
        assert!(pacing.gap_ms <= 15);
        assert!(pacing.terminal_post_delay_ms >= 4);
        assert!(pacing.browser_post_delay_ms >= 6);
        assert!(pacing.draftjs_post_delay_ms >= 8);
        assert!(pacing.estimated_replace_ms > 0.0);
    }

    #[test]
    fn test_mixed_refresh_rate_fail_safe() {
        // simulation of laptop 120hz and external 60hz without active focus
        let display = DisplayEnvironment {
            monitors: vec![
                MonitorInfo {
                    name: "eDP-1".to_string(),
                    width: 1920,
                    height: 1080,
                    refresh_rate: 120.0,
                    is_active_focus: false,
                    is_primary: true,
                },
                MonitorInfo {
                    name: "HDMI-A-1".to_string(),
                    width: 2560,
                    height: 1440,
                    refresh_rate: 60.0,
                    is_active_focus: false,
                    is_primary: false,
                },
            ],
            effective_refresh_rate: 60.0,
            frame_budget_ms: 16.67,
            is_mixed_refresh_rate: true,
            detection_method: "Test Multi".to_string(),
        };

        let latency = LatencyBenchmark {
            uinput_accessible: true,
            uinput_write_us: 15.0,
            measured_rtt_ms: 2.5,
            measured_jitter_ms: 0.8,
            keyboard_repeat_delay_ms: 600,
            keyboard_repeat_interval_ms: 25,
            scheduler_preempt: "PREEMPT".to_string(),
            system_load: 1.2,
        };

        let pacing = calculate_pacing(&display, &latency);
        assert!(pacing.explanation.contains("Fail-Safe đa màn hình"));
        assert_eq!(display.effective_refresh_rate, 60.0);
        assert!(pacing.draftjs_post_delay_ms >= 16);
    }

    #[test]
    fn test_restart_fcitx5_daemon_functionality() {
        // verify restart succeeds and leaves fcitx5 running
        let ok = restart_fcitx5_daemon();
        assert!(ok);
        let status = check_daemon();
        assert!(status.fcitx5_running);
        assert!(!status.fcitx5_pids.is_empty());
    }

    #[test]
    fn test_apply_pacing_profile() {
        let pacing = checks::pacing_calc::PacingRecommendation {
            gap_ms: 5,
            terminal_post_delay_ms: 10,
            browser_post_delay_ms: 14,
            draftjs_post_delay_ms: 18,
            heavy_app_post_delay_ms: 22,
            estimated_replace_ms: 28.0,
            flicker_risk: "Thấp".to_string(),
            explanation: "test profile".to_string(),
        };

        let res = apply_pacing_profile(&pacing);
        assert!(res.is_ok());

        let cfg = clak_engine::config::ClakConfig::load();
        assert_eq!(cfg.pacing.preset, "custom");
        assert_eq!(cfg.pacing.draftjs_delay_ms, 18);
        assert_eq!(cfg.pacing.draftjs_gap_ms, 5);
        assert_eq!(cfg.pacing.terminal_delay_ms, 10);
        assert_eq!(cfg.pacing.heavy_app_delay_ms, 22);
    }
}
