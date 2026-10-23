use crate::DiagnosticReport;

pub fn format_markdown(report: &DiagnosticReport) -> String {
    let mut out = String::new();
    out.push_str("# Clak Doctor: Báo Cáo Chẩn Đoán Hệ Thống\n\n");

    out.push_str("## 1. Môi Trường Hệ Thống\n");
    out.push_str(&format!("- Hệ điều hành: {}\n", report.system.os_name));
    out.push_str(&format!(
        "- Nhân Linux: {} ({})\n",
        report.system.kernel_release, report.system.architecture
    ));
    out.push_str(&format!(
        "- Session: {} (Desktop: {})\n\n",
        report.system.session_type, report.system.desktop_environment
    ));

    out.push_str("## 2. Quyền Hạn & Thiết Bị\n");
    out.push_str(&format!(
        "- `/dev/uinput`: {}\n",
        if report.permissions.uinput_writable {
            "Khả dụng (có quyền ghi)"
        } else if report.permissions.xtest_available {
            "Thiếu quyền ghi (Đang dùng XTest fallback trên X11/XWayland)"
        } else {
            "Thiếu quyền ghi"
        }
    ));
    out.push_str(&format!(
        "- XTest Fallback: {}\n",
        if report.permissions.xtest_available {
            "Khả dụng"
        } else {
            "Không khả dụng"
        }
    ));
    out.push_str(&format!(
        "- `/dev/input/event*`: Đọc được {}/{} thiết bị\n\n",
        report.permissions.input_readable_count, report.permissions.input_total_count
    ));

    out.push_str("## 3. Trạng Thái Fcitx5 Daemon\n");
    if report.daemon.fcitx5_running {
        let pids: Vec<String> = report
            .daemon
            .fcitx5_pids
            .iter()
            .map(|p| p.to_string())
            .collect();
        out.push_str(&format!(
            "- Tiến trình: Đang chạy (PID: {})\n",
            pids.join(", ")
        ));
    } else {
        out.push_str("- Tiến trình: Chưa khởi chạy\n");
    }
    out.push_str(&format!(
        "- Bộ gõ đang kích hoạt: {}\n\n",
        report
            .daemon
            .current_im
            .as_deref()
            .unwrap_or("Không xác định")
    ));

    out.push_str("## 4. Kiểm Tra Bộ Nhớ & Thư Viện Clak\n");
    out.push_str(&format!(
        "- Thư viện libclak: {}\n",
        if report.memory.clak_loaded {
            "Đã được nạp vào tiến trình"
        } else {
            "Chưa được nạp"
        }
    ));
    if let Some(ref path) = report.memory.mapped_path {
        out.push_str(&format!("- Đường dẫn nạp: `{}`\n", path));
    }
    if report.memory.is_deleted_inode {
        out.push_str(
            "- Cảnh báo bộ nhớ: Fcitx5 đang giữ file binary cũ (deleted inode) trong RAM\n",
        );
    }
    out.push('\n');

    out.push_str("## 5. Cấu Hình & Khởi Động\n");
    out.push_str(&format!(
        "- File cấu hình: {}\n",
        if report.config.config_file_exists {
            format!("Tồn tại (`{}`)", report.config.config_path.display())
        } else {
            "Chưa tạo file riêng (dùng mặc định)".to_string()
        }
    ));
    if report.config.log_file_exists {
        out.push_str(&format!(
            "- Nhật ký debug: Có tồn tại ({:.1} KB)\n",
            report.config.log_size_kb.unwrap_or(0.0)
        ));
    } else {
        out.push_str("- Nhật ký debug: Chưa ghi log\n");
    }
    out.push_str(&format!(
        "- Khởi động cùng hệ thống: {}\n\n",
        if report.autostart.is_enabled {
            "Đã bật"
        } else {
            "Chưa bật"
        }
    ));

    out.push_str("## 6. Biến Môi Trường Input Method\n");
    out.push_str(&format!(
        "- GTK_IM_MODULE: `{}`\n",
        report
            .environment
            .gtk_im_module
            .as_deref()
            .unwrap_or("(không đặt)")
    ));
    out.push_str(&format!(
        "- QT_IM_MODULE: `{}`\n",
        report
            .environment
            .qt_im_module
            .as_deref()
            .unwrap_or("(không đặt)")
    ));
    out.push_str(&format!(
        "- XMODIFIERS: `{}`\n",
        report
            .environment
            .xmodifiers
            .as_deref()
            .unwrap_or("(không đặt)")
    ));
    out.push_str(&format!(
        "- File 99-clak-im.conf: {}\n\n",
        if report.environment.env_file_exists {
            "Đã có"
        } else {
            "Chưa có"
        }
    ));

    out.push_str("## 7. Thư Viện Frontend Fcitx5\n");
    out.push_str(&format!(
        "- GTK 3: {}\n",
        report
            .frontends
            .gtk3_path
            .as_deref()
            .unwrap_or("Không tìm thấy")
    ));
    out.push_str(&format!(
        "- GTK 4: {}\n",
        report
            .frontends
            .gtk4_path
            .as_deref()
            .unwrap_or("Không tìm thấy")
    ));
    out.push_str(&format!(
        "- Qt 5: {}\n",
        report
            .frontends
            .qt5_path
            .as_deref()
            .unwrap_or("Không tìm thấy")
    ));
    out.push_str(&format!(
        "- Qt 6: {}\n\n",
        report
            .frontends
            .qt6_path
            .as_deref()
            .unwrap_or("Không tìm thấy")
    ));

    out.push_str("## 8. Cấu Hình Fcitx5 & Tương Thích Ứng Dụng\n");
    out.push_str(&format!(
        "- Fcitx5 Profile: {}\n",
        if report.profile.clak_in_profile {
            if report.profile.is_default_im {
                "Đã thêm và là bộ gõ mặc định (OK)"
            } else {
                "Đã thêm nhưng chưa đặt làm mặc định"
            }
        } else {
            "Chưa thêm Clak vào danh sách bộ gõ"
        }
    ));
    if report.wps.installed {
        out.push_str(&format!(
            "- WPS Office: {}\n",
            if report.wps.configured {
                "Đã tối ưu QT_IM_MODULE=fcitx (OK)"
            } else {
                "Chưa tối ưu cho Wayland (cần gán QT_IM_MODULE=fcitx)"
            }
        ));
    }
    if report.jetbrains.installed {
        out.push_str(&format!(
            "- JetBrains IDEs: {}\n",
            if report.jetbrains.unconfigured_ides.is_empty() {
                format!(
                    "Đã tối ưu XToolkit ({})",
                    report.jetbrains.configured_ides.join(", ")
                )
            } else {
                format!(
                    "Chưa tối ưu: {}",
                    report.jetbrains.unconfigured_ides.join(", ")
                )
            }
        ));
    }
    if report.kde.is_kde {
        out.push_str(&format!(
            "- KDE Plasma: {}\n",
            if report.kde.virtual_keyboard_configured {
                "VirtualKeyboard đã trỏ về Fcitx5 (OK)"
            } else {
                "VirtualKeyboard chưa trỏ về Fcitx5 (cần cấu hình kwinrc)"
            }
        ));
    }
    out.push('\n');

    if !report.conflicts.is_empty() {
        out.push_str("## Ứng Dụng Xung Đột\n");
        for conf in &report.conflicts {
            out.push_str(&format!(
                "- **{}** (PID {}): {}\n",
                conf.app_name, conf.pid, conf.description
            ));
            out.push_str(&format!(
                "  * Hướng khắc phục: `{}`\n",
                conf.resolution_hint
            ));
        }
        out.push('\n');
    }

    out.push_str("## 9. Màn Hình & VSync Budget\n");
    out.push_str(&format!(
        "- Phương thức phát hiện: {}\n",
        report.display.detection_method
    ));
    out.push_str(&format!(
        "- Tần số quét hiệu dụng: {:.1} Hz (Frame budget: {:.2} ms)\n",
        report.display.effective_refresh_rate, report.display.frame_budget_ms
    ));
    if report.display.is_mixed_refresh_rate {
        out.push_str(
            "- Cảnh báo đa màn hình: Phát hiện hỗn hợp tần số quét (khóa mốc an toàn tối thiểu)\n",
        );
    }
    out.push_str("- Danh sách màn hình:\n");
    for mon in &report.display.monitors {
        let focus_tag = if mon.is_active_focus {
            " [Active Focus]"
        } else {
            ""
        };
        let primary_tag = if mon.is_primary { " (Primary)" } else { "" };
        out.push_str(&format!(
            "  * `{}`: {}x{} @ {:.1} Hz{}{}\n",
            mon.name, mon.width, mon.height, mon.refresh_rate, primary_tag, focus_tag
        ));
    }
    out.push('\n');

    out.push_str("## 10. Độ Trễ Phần Cứng & Pacing Đề Xuất\n");
    out.push_str(&format!(
        "- Phản hồi Uinput: {:.2} ms (Jitter: ±{:.2} ms, Write: {:.1} µs)\n",
        report.latency.measured_rtt_ms,
        report.latency.measured_jitter_ms,
        report.latency.uinput_write_us
    ));
    out.push_str(&format!(
        "- Bàn phím lặp lại: Delay {} ms, Interval {} ms\n",
        report.latency.keyboard_repeat_delay_ms, report.latency.keyboard_repeat_interval_ms
    ));
    out.push_str(&format!(
        "- Tải hệ thống: {:.2} (Preempt: {})\n",
        report.latency.system_load, report.latency.scheduler_preempt
    ));
    out.push_str(&format!(
        "- Nguy cơ nhấp nháy: {}\n",
        report.pacing.flicker_risk
    ));
    out.push_str(&format!("  * {}\n", report.pacing.explanation));
    out.push_str("- Bảng cấu hình Pacing tối ưu:\n");
    out.push_str(&format!("  * `gap_ms`: {} ms\n", report.pacing.gap_ms));
    out.push_str(&format!(
        "  * `terminal_post_delay_ms`: {} ms\n",
        report.pacing.terminal_post_delay_ms
    ));
    out.push_str(&format!(
        "  * `browser_post_delay_ms`: {} ms\n",
        report.pacing.browser_post_delay_ms
    ));
    out.push_str(&format!(
        "  * `draftjs_post_delay_ms`: {} ms\n",
        report.pacing.draftjs_post_delay_ms
    ));
    out.push_str(&format!(
        "  * `heavy_app_post_delay_ms`: {} ms\n",
        report.pacing.heavy_app_post_delay_ms
    ));
    out.push('\n');

    out.push_str("## Kết Luận & Hướng Xử Lý\n");
    if report.issues.is_empty() && report.warnings.is_empty() {
        out.push_str("Tất cả kiểm tra đều hoàn hảo. Clak sẵn sàng hoạt động tối ưu.\n");
    } else {
        if !report.issues.is_empty() {
            out.push_str("### Cần Xử Lý (Lỗi nghiêm trọng):\n");
            for (idx, iss) in report.issues.iter().enumerate() {
                out.push_str(&format!("{}. {}\n", idx + 1, iss.message));
                if let Some(ref fix) = iss.fix_command {
                    out.push_str(&format!("   * Lệnh sửa: `{}`\n", fix));
                }
            }
        }
        if !report.warnings.is_empty() {
            out.push_str("\n### Lưu Ý (Khuyến nghị):\n");
            for (idx, warn) in report.warnings.iter().enumerate() {
                out.push_str(&format!("{}. {}\n", idx + 1, warn.message));
                if let Some(ref fix) = warn.fix_command {
                    out.push_str(&format!("   * Khuyến nghị: `{}`\n", fix));
                }
            }
        }
    }

    out
}
