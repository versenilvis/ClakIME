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
        } else {
            "Thiếu quyền ghi"
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
        out.push_str(&format!("- Tiến trình: Đang chạy (PID: {})\n", pids.join(", ")));
    } else {
        out.push_str("- Tiến trình: Chưa khởi chạy\n");
    }
    out.push_str(&format!(
        "- Bộ gõ đang kích hoạt: {}\n\n",
        report.daemon.current_im.as_deref().unwrap_or("Không xác định")
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
        out.push_str("- Cảnh báo bộ nhớ: Fcitx5 đang giữ file binary cũ (deleted inode) trong RAM\n");
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
        report.environment.gtk_im_module.as_deref().unwrap_or("(không đặt)")
    ));
    out.push_str(&format!(
        "- QT_IM_MODULE: `{}`\n",
        report.environment.qt_im_module.as_deref().unwrap_or("(không đặt)")
    ));
    out.push_str(&format!(
        "- XMODIFIERS: `{}`\n",
        report.environment.xmodifiers.as_deref().unwrap_or("(không đặt)")
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
        report.frontends.gtk3_path.as_deref().unwrap_or("Không tìm thấy")
    ));
    out.push_str(&format!(
        "- GTK 4: {}\n",
        report.frontends.gtk4_path.as_deref().unwrap_or("Không tìm thấy")
    ));
    out.push_str(&format!(
        "- Qt 5: {}\n",
        report.frontends.qt5_path.as_deref().unwrap_or("Không tìm thấy")
    ));
    out.push_str(&format!(
        "- Qt 6: {}\n\n",
        report.frontends.qt6_path.as_deref().unwrap_or("Không tìm thấy")
    ));

    if !report.conflicts.is_empty() {
        out.push_str("## Ứng Dụng Xung Đột\n");
        for conf in &report.conflicts {
            out.push_str(&format!("- **{}** (PID {}): {}\n", conf.app_name, conf.pid, conf.description));
            out.push_str(&format!("  * Hướng khắc phục: `{}`\n", conf.resolution_hint));
        }
        out.push('\n');
    }

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
