use clak_diagnostics::run_diagnostics;

pub fn execute(markdown: bool, json: bool) {
    let report = run_diagnostics();

    if json {
        match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{}", s),
            Err(e) => eprintln!("Lỗi chuyển đổi JSON: {}", e),
        }
        return;
    }

    if markdown {
        println!("{}", report.to_markdown());
        return;
    }

    println!("=== CLAK DOCTOR: HỆ THỐNG CHẨN ĐOÁN MÔI TRƯỜNG ===");

    println!("\n[1/5] Môi trường hệ thống:");
    println!(
        "  • Hệ điều hành: {} ({})",
        report.system.os_name, report.system.architecture
    );
    println!("  • Nhân Linux: {}", report.system.kernel_release);
    println!(
        "  • Session: {} (Desktop: {})",
        report.system.session_type, report.system.desktop_environment
    );

    println!("\n[2/5] Thiết bị ảo và Quyền hạn:");
    if report.permissions.uinput_writable {
        println!("  • /dev/uinput: \x1b[32m[OK]\x1b[0m Khả dụng và có quyền ghi");
    } else {
        println!("  • /dev/uinput: \x1b[31m[THIẾU QUYỀN]\x1b[0m Người dùng chưa có quyền ghi vào /dev/uinput");
    }
    if report.permissions.input_readable_count > 0 {
        println!(
            "  • /dev/input (Mouse Tracking): \x1b[32m[OK]\x1b[0m Đọc được {}/{} thiết bị",
            report.permissions.input_readable_count, report.permissions.input_total_count
        );
    } else {
        println!("  • /dev/input (Mouse Tracking): \x1b[33m[CẢNH BÁO]\x1b[0m Không đọc được thiết bị input");
    }

    println!("\n[3/5] Trạng thái Daemon Fcitx5:");
    if report.daemon.fcitx5_running {
        let pids_str = report
            .daemon
            .fcitx5_pids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "  • Tiến trình Fcitx5: \x1b[32m[OK]\x1b[0m Đang chạy (PID {})",
            pids_str
        );
    } else {
        println!("  • Tiến trình Fcitx5: \x1b[31m[LỖI]\x1b[0m Fcitx5 chưa khởi chạy");
    }
    if let Some(ref im) = report.daemon.current_im {
        println!("  • Bộ gõ hiện tại: {}", im);
    } else {
        println!("  • fcitx5-remote: Không thể kết nối tới Fcitx5");
    }

    println!("\n[4/5] Kiểm tra Binary & Nạp bộ nhớ (RAM):");
    if report.memory.clak_loaded {
        let path = report.memory.mapped_path.as_deref().unwrap_or("libclak.so");
        if report.memory.is_deleted_inode {
            println!(
                "  • Bộ nhớ: \x1b[33m[CẢNH BÁO]\x1b[0m Fcitx5 đang giữ inode CŨ trong RAM: {}",
                path
            );
        } else {
            println!("  • Bộ nhớ: \x1b[32m[OK]\x1b[0m Đang nạp {}", path);
        }
    } else if report.daemon.fcitx5_running {
        println!("  • Bộ nhớ: \x1b[31m[LỖI]\x1b[0m libclak.so chưa được Fcitx5 nạp vào tiến trình");
    }

    println!("\n[5/5] Cấu hình và Nhật ký (Log):");
    if report.config.config_file_exists {
        println!(
            "  • File cấu hình: \x1b[32m[OK]\x1b[0m {}",
            report.config.config_path.display()
        );
    } else {
        println!("  • File cấu hình: [MẶC ĐỊNH] Chưa tạo file riêng, đang dùng giá trị mặc định");
    }
    if report.config.log_file_exists {
        println!(
            "  • File log: \x1b[32m[OK]\x1b[0m {} ({:.1} KB)",
            report.config.log_path.display(),
            report.config.log_size_kb.unwrap_or(0.0)
        );
    } else {
        println!(
            "  • File log: Chưa kích hoạt ghi log debug vào {}",
            report.config.log_path.display()
        );
    }
    if report.autostart.is_enabled {
        println!("  • Khởi động cùng hệ thống: \x1b[32m[OK]\x1b[0m Đã kích hoạt");
    } else {
        println!("  • Khởi động cùng hệ thống: \x1b[33m[CẢNH BÁO]\x1b[0m Chưa kích hoạt tự động chạy khi đăng nhập");
    }

    if !report.conflicts.is_empty() {
        println!("\nỨng dụng xung đột:");
        for conf in &report.conflicts {
            println!("  • {} (PID {}): {}", conf.app_name, conf.pid, conf.description);
        }
    }

    println!("\n{}", "=".repeat(55));
    if report.issues.is_empty() && report.warnings.is_empty() {
        println!("\x1b[32m✔ TẤT CẢ KIỂM TRA ĐỀU HOÀN HẢO!\x1b[0m Clak đã sẵn sàng hoạt động tối ưu.");
    } else {
        if !report.issues.is_empty() {
            println!(
                "\n\x1b[31m❌ CẦN XỬ LÝ ({} vấn đề):\x1b[0m",
                report.issues.len()
            );
            for (idx, iss) in report.issues.iter().enumerate() {
                println!("  {}. {}", idx + 1, iss.message);
                if let Some(ref fix) = iss.fix_command {
                    println!("     Lệnh khắc phục: {}", fix);
                }
            }
        }
        if !report.warnings.is_empty() {
            println!(
                "\n\x1b[33m⚠️  LƯU Ý ({} khuyến nghị):\x1b[0m",
                report.warnings.len()
            );
            for (idx, warn) in report.warnings.iter().enumerate() {
                println!("  {}. {}", idx + 1, warn.message);
                if let Some(ref fix) = warn.fix_command {
                    println!("     Khuyến nghị: {}", fix);
                }
            }
        }
    }
    println!("{}\n", "=".repeat(55));
}
