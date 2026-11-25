use clak_diagnostics::DiagnosticReport;
use crate::DoctorLineData;

pub fn format_doctor_lines(report: &DiagnosticReport, time_tag: &str) -> Vec<DoctorLineData> {
    let mut lines = Vec::new();
    let header_color = slint::Color::from_argb_u8(255, 56, 189, 248);
    let normal_color = slint::Color::from_argb_u8(255, 212, 212, 216);
    let ok_color = slint::Color::from_argb_u8(255, 52, 211, 153);
    let warn_color = slint::Color::from_argb_u8(255, 251, 191, 36);
    let error_color = slint::Color::from_argb_u8(255, 248, 113, 113);
    let code_color = slint::Color::from_argb_u8(255, 167, 139, 250);
    let dim_color = slint::Color::from_argb_u8(255, 113, 113, 122);

    // danh sach cac file va checkpoint kiem tra theo thu tu
    if report.permissions.uinput_writable {
        lines.push(DoctorLineData {
            text: format!("[{}] SUCCESS | /dev/uinput (Quyền ghi khả dụng)", time_tag).into(),
            color: ok_color,
            bold: false,
        });
    } else if report.permissions.xtest_available {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] WARNING | /dev/uinput (Thiếu quyền ghi, đang dùng XTest fallback)",
                time_tag
            )
            .into(),
            color: warn_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!("[{}] FAILED  | /dev/uinput (Không có quyền ghi)", time_tag).into(),
            color: error_color,
            bold: true,
        });
    }

    if report.permissions.input_readable_count > 0 {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | /dev/input (Đọc được {}/{} thiết bị)",
                time_tag,
                report.permissions.input_readable_count,
                report.permissions.input_total_count
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] FAILED  | /dev/input (Không đọc được thiết bị input)",
                time_tag
            )
            .into(),
            color: error_color,
            bold: true,
        });
    }

    if report.daemon.fcitx5_running && report.memory.clak_loaded && !report.memory.is_deleted_inode
    {
        let pids = report
            .daemon
            .fcitx5_pids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | /proc/{}/maps (Fcitx5 nạp libclak.so)",
                time_tag, pids
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else if report.daemon.fcitx5_running
        && report.memory.clak_loaded
        && report.memory.is_deleted_inode
    {
        let pids = report
            .daemon
            .fcitx5_pids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(DoctorLineData {
            text: format!(
                "[{}] FAILED  | /proc/{}/maps (Inode cũ trong RAM)",
                time_tag, pids
            )
            .into(),
            color: error_color,
            bold: true,
        });
    } else if report.daemon.fcitx5_running {
        let pids = report
            .daemon
            .fcitx5_pids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(DoctorLineData {
            text: format!(
                "[{}] FAILED  | /proc/{}/maps (Chưa nạp libclak.so)",
                time_tag, pids
            )
            .into(),
            color: error_color,
            bold: true,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] FAILED  | fcitx5 daemon (Không tìm thấy tiến trình)",
                time_tag
            )
            .into(),
            color: error_color,
            bold: true,
        });
    }

    if report.config.config_file_exists {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | {}",
                time_tag,
                report.config.config_path.display()
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | {} (Mặc định)",
                time_tag,
                report.config.config_path.display()
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    }

    if report.config.log_file_exists {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | {} ({:.1} KB)",
                time_tag,
                report.config.log_path.display(),
                report.config.log_size_kb.unwrap_or(0.0)
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | {} (Chưa bật debug)",
                time_tag,
                report.config.log_path.display()
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    }

    if report.autostart.is_enabled {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | ~/.config/autostart/org.fcitx.Fcitx5.desktop",
                time_tag
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] FAILED  | ~/.config/autostart/org.fcitx.Fcitx5.desktop (Chưa kích hoạt)",
                time_tag
            )
            .into(),
            color: error_color,
            bold: true,
        });
    }

    let gtk_im = report.environment.gtk_im_module.as_deref().unwrap_or("");
    if gtk_im == "ibus" {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] FAILED  | GTK_IM_MODULE=ibus (Xung đột với IBus)",
                time_tag
            )
            .into(),
            color: error_color,
            bold: true,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | GTK_IM_MODULE ({})",
                time_tag,
                if gtk_im.is_empty() {
                    "Mặc định Wayland"
                } else {
                    gtk_im
                }
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    }

    let has_gtk = report.frontends.gtk3 || report.frontends.gtk4;
    if has_gtk {
        lines.push(DoctorLineData {
            text: format!("[{}] SUCCESS | Frontend GTK Fcitx5 (Khả dụng)", time_tag).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] FAILED  | Frontend GTK Fcitx5 (Thiếu module GTK3/4)",
                time_tag
            )
            .into(),
            color: warn_color,
            bold: true,
        });
    }

    if report.conflicts.is_empty() {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] SUCCESS | Kiểm tra xung đột (Không phát hiện xung đột)",
                time_tag
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "[{}] FAILED  | Kiểm tra xung đột (Phát hiện {} ứng dụng/daemon)",
                time_tag,
                report.conflicts.len()
            )
            .into(),
            color: error_color,
            bold: true,
        });
    }

    lines.push(DoctorLineData {
        text: "Đã quét xong!".into(),
        color: ok_color,
        bold: true,
    });
    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });

    lines.push(DoctorLineData {
        text: "[1/10] Môi trường hệ thống:".into(),
        color: header_color,
        bold: true,
    });
    lines.push(DoctorLineData {
        text: format!(
            "  • Hệ điều hành: {} ({})",
            report.system.os_name, report.system.architecture
        )
        .into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: format!("  • Nhân Linux: {}", report.system.kernel_release).into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: format!(
            "  • Session: {} (Desktop: {})",
            report.system.session_type, report.system.desktop_environment
        )
        .into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });

    lines.push(DoctorLineData {
        text: "[2/10] Thiết bị ảo và Quyền hạn:".into(),
        color: header_color,
        bold: true,
    });
    if report.permissions.uinput_writable {
        lines.push(DoctorLineData {
            text: "  • /dev/uinput: [OK] Khả dụng (chống nuốt chữ Kitty, Ghostty, Chrome, VSCode)"
                .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • /dev/uinput: [THIẾU QUYỀN] Người dùng chưa có quyền ghi vào /dev/uinput"
                .into(),
            color: error_color,
            bold: true,
        });
    }
    if report.permissions.input_readable_count > 0 {
        lines.push(DoctorLineData {
            text: format!(
                "  • /dev/input (Mouse Tracking): [OK] Đọc được {}/{} thiết bị",
                report.permissions.input_readable_count, report.permissions.input_total_count
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • /dev/input (Mouse Tracking): [CẢNH BÁO] Không đọc được thiết bị input"
                .into(),
            color: warn_color,
            bold: true,
        });
    }
    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });

    lines.push(DoctorLineData {
        text: "[3/10] Trạng thái Daemon Fcitx5:".into(),
        color: header_color,
        bold: true,
    });
    if report.daemon.fcitx5_running {
        let pids_str = report
            .daemon
            .fcitx5_pids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(DoctorLineData {
            text: format!("  • Tiến trình Fcitx5: [OK] Đang chạy (PID {})", pids_str).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • Tiến trình Fcitx5: [LỖI] Fcitx5 chưa khởi chạy".into(),
            color: error_color,
            bold: true,
        });
    }
    if let Some(ref im) = report.daemon.current_im {
        let is_clak = im == "clak";
        lines.push(DoctorLineData {
            text: format!("  • Bộ gõ hiện tại: {}", im).into(),
            color: if is_clak { ok_color } else { warn_color },
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • fcitx5-remote: Không thể kết nối tới Fcitx5".into(),
            color: warn_color,
            bold: false,
        });
    }
    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });

    lines.push(DoctorLineData {
        text: "[4/10] Kiểm tra Binary & Nạp bộ nhớ (RAM):".into(),
        color: header_color,
        bold: true,
    });
    if report.memory.clak_loaded {
        let path = report.memory.mapped_path.as_deref().unwrap_or("libclak.so");
        if report.memory.is_deleted_inode {
            lines.push(DoctorLineData {
                text: format!(
                    "  • Bộ nhớ: [CẢNH BÁO] Fcitx5 đang giữ inode CŨ trong RAM: {}",
                    path
                )
                .into(),
                color: warn_color,
                bold: true,
            });
        } else {
            lines.push(DoctorLineData {
                text: format!("  • Bộ nhớ: [OK] Đang nạp {}", path).into(),
                color: ok_color,
                bold: false,
            });
        }
    } else if report.daemon.fcitx5_running {
        lines.push(DoctorLineData {
            text: "  • Bộ nhớ: [LỖI] libclak.so chưa được Fcitx5 nạp vào tiến trình".into(),
            color: error_color,
            bold: true,
        });
    }
    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });

    lines.push(DoctorLineData {
        text: "[5/10] Cấu hình và Nhật ký (Log):".into(),
        color: header_color,
        bold: true,
    });
    if report.config.config_file_exists {
        lines.push(DoctorLineData {
            text: format!(
                "  • File cấu hình: [OK] {}",
                report.config.config_path.display()
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • File cấu hình: [MẶC ĐỊNH] Chưa tạo file riêng, đang dùng giá trị mặc định"
                .into(),
            color: normal_color,
            bold: false,
        });
    }
    if report.config.log_file_exists {
        lines.push(DoctorLineData {
            text: format!(
                "  • File log: [OK] {} ({:.1} KB)",
                report.config.log_path.display(),
                report.config.log_size_kb.unwrap_or(0.0)
            )
            .into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!(
                "  • File log: Chưa kích hoạt ghi log debug vào {}",
                report.config.log_path.display()
            )
            .into(),
            color: dim_color,
            bold: false,
        });
    }
    if report.autostart.is_enabled {
        lines.push(DoctorLineData {
            text: "  • Khởi động cùng hệ thống: [OK] Đã kích hoạt".into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text:
                "  • Khởi động cùng hệ thống: [CẢNH BÁO] Chưa kích hoạt tự động chạy khi đăng nhập"
                    .into(),
            color: warn_color,
            bold: true,
        });
    }

    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "[6/10] Biến Môi Trường Input Method:".into(),
        color: header_color,
        bold: true,
    });
    let gtk_str = report
        .environment
        .gtk_im_module
        .as_deref()
        .unwrap_or("(chưa đặt)");
    let is_gtk_bad = gtk_str == "ibus";
    lines.push(DoctorLineData {
        text: format!(
            "  • GTK_IM_MODULE: [{}] {}",
            if is_gtk_bad { "XUNG ĐỘT" } else { "OK" },
            gtk_str
        )
        .into(),
        color: if is_gtk_bad { error_color } else { ok_color },
        bold: is_gtk_bad,
    });
    let qt_str = report
        .environment
        .qt_im_module
        .as_deref()
        .unwrap_or("(chưa đặt)");
    let is_qt_bad = qt_str == "ibus";
    lines.push(DoctorLineData {
        text: format!(
            "  • QT_IM_MODULE: [{}] {}",
            if is_qt_bad { "XUNG ĐỘT" } else { "OK" },
            qt_str
        )
        .into(),
        color: if is_qt_bad { error_color } else { ok_color },
        bold: is_qt_bad,
    });
    let xmod_str = report
        .environment
        .xmodifiers
        .as_deref()
        .unwrap_or("(chưa đặt)");
    let is_xmod_bad = xmod_str.contains("ibus");
    lines.push(DoctorLineData {
        text: format!(
            "  • XMODIFIERS: [{}] {}",
            if is_xmod_bad { "CẢNH BÁO" } else { "OK" },
            xmod_str
        )
        .into(),
        color: if is_xmod_bad { warn_color } else { ok_color },
        bold: is_xmod_bad,
    });
    if let Some(ref sdl) = report.environment.sdl_im_module {
        let is_bad = sdl.contains("ibus");
        lines.push(DoctorLineData {
            text: format!(
                "  • SDL_IM_MODULE: [{}] {}",
                if is_bad { "CẢNH BÁO" } else { "OK" },
                sdl
            )
            .into(),
            color: if is_bad { warn_color } else { ok_color },
            bold: is_bad,
        });
    }
    if let Some(ref glfw) = report.environment.glfw_im_module {
        let is_bad = glfw.contains("ibus");
        lines.push(DoctorLineData {
            text: format!(
                "  • GLFW_IM_MODULE: [{}] {}",
                if is_bad { "CẢNH BÁO" } else { "OK" },
                glfw
            )
            .into(),
            color: if is_bad { warn_color } else { ok_color },
            bold: is_bad,
        });
    }
    if report.environment.env_file_exists {
        lines.push(DoctorLineData {
            text: "  • File 99-clak-im.conf: [OK] Đã cấu hình".into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • File 99-clak-im.conf: [LƯU Ý] Chưa tạo file ~/.config/environment.d/99-clak-im.conf".into(),
            color: warn_color,
            bold: true,
        });
    }

    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "[7/10] Thư Viện Frontend Fcitx5:".into(),
        color: header_color,
        bold: true,
    });
    if let Some(ref p) = report.frontends.gtk3_path {
        lines.push(DoctorLineData {
            text: format!("  • GTK 3: [OK] {}", p).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • GTK 3: [CHƯA CÓ] Chưa tìm thấy module frontend GTK3".into(),
            color: warn_color,
            bold: false,
        });
    }
    if let Some(ref p) = report.frontends.gtk4_path {
        lines.push(DoctorLineData {
            text: format!("  • GTK 4: [OK] {}", p).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • GTK 4: [CHƯA CÓ] Chưa tìm thấy module frontend GTK4".into(),
            color: warn_color,
            bold: false,
        });
    }
    if let Some(ref p) = report.frontends.qt5_path {
        lines.push(DoctorLineData {
            text: format!("  • Qt 5: [OK] {}", p).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • Qt 5: [CHƯA CÓ] Chưa tìm thấy plugin frontend Qt5".into(),
            color: warn_color,
            bold: false,
        });
    }
    if let Some(ref p) = report.frontends.qt6_path {
        lines.push(DoctorLineData {
            text: format!("  • Qt 6: [OK] {}", p).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • Qt 6: [CHƯA CÓ] Chưa tìm thấy plugin frontend Qt6".into(),
            color: warn_color,
            bold: false,
        });
    }

    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "[8/10] Cấu Hình Fcitx5 & Tương Thích Ứng Dụng:".into(),
        color: header_color,
        bold: true,
    });
    if report.profile.clak_in_profile {
        if report.profile.is_default_im {
            lines.push(DoctorLineData {
                text: "  • Fcitx5 Profile: [OK] Đã kích hoạt và là bộ gõ mặc định".into(),
                color: ok_color,
                bold: false,
            });
        } else {
            lines.push(DoctorLineData {
                text: "  • Fcitx5 Profile: [LƯU Ý] Đã thêm nhưng chưa đặt DefaultIM=clak".into(),
                color: warn_color,
                bold: false,
            });
        }
    } else {
        lines.push(DoctorLineData {
            text: "  • Fcitx5 Profile: [CHƯA CÓ] Chưa thêm Clak vào danh sách bộ gõ".into(),
            color: error_color,
            bold: true,
        });
    }
    if report.wps.installed {
        if report.wps.configured {
            lines.push(DoctorLineData {
                text: "  • WPS Office: [OK] Đã tối ưu biến QT_IM_MODULE=fcitx".into(),
                color: ok_color,
                bold: false,
            });
        } else {
            lines.push(DoctorLineData {
                text:
                    "  • WPS Office: [CẦN TỐI ƯU] Chạy Qt5 nội bộ trên XWayland (Dùng nút Tối ưu)"
                        .into(),
                color: warn_color,
                bold: true,
            });
        }
    }
    if report.jetbrains.installed {
        if report.jetbrains.unconfigured_ides.is_empty() {
            lines.push(DoctorLineData {
                text: format!(
                    "  • JetBrains IDEs: [OK] Đã cấu hình XToolkit ({})",
                    report.jetbrains.configured_ides.join(", ")
                )
                .into(),
                color: ok_color,
                bold: false,
            });
        } else {
            lines.push(DoctorLineData {
                text: format!(
                    "  • JetBrains IDEs: [CẦN TỐI ƯU] Chưa cấu hình: {}",
                    report.jetbrains.unconfigured_ides.join(", ")
                )
                .into(),
                color: warn_color,
                bold: true,
            });
        }
    }

    if !report.conflicts.is_empty() {
        lines.push(DoctorLineData {
            text: "".into(),
            color: normal_color,
            bold: false,
        });
        lines.push(DoctorLineData {
            text: "Ứng dụng xung đột:".into(),
            color: warn_color,
            bold: true,
        });
        for conf in &report.conflicts {
            lines.push(DoctorLineData {
                text: format!(
                    "  • {} (PID {}): {}",
                    conf.app_name, conf.pid, conf.description
                )
                .into(),
                color: warn_color,
                bold: false,
            });
            lines.push(DoctorLineData {
                text: format!("    Khắc phục: {}", conf.resolution_hint).into(),
                color: code_color,
                bold: false,
            });
        }
    }

    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "[9/10] Màn Hình & VSync Frame Budget:".into(),
        color: header_color,
        bold: true,
    });
    lines.push(DoctorLineData {
        text: format!(
            "  • Tần số quét hiệu dụng: {:.1} Hz (Chu kỳ khung hình: {:.2} ms)",
            report.display.effective_refresh_rate, report.display.frame_budget_ms
        )
        .into(),
        color: ok_color,
        bold: false,
    });
    if report.display.is_mixed_refresh_rate {
        lines.push(DoctorLineData {
            text: format!(
                "  • Đa màn hình: [HỖN HỢP] Khóa mốc an toàn tối thiểu {:.0} Hz",
                report.display.effective_refresh_rate
            )
            .into(),
            color: warn_color,
            bold: true,
        });
    }
    for mon in &report.display.monitors {
        let tag = if mon.is_active_focus {
            " [Active Focus]"
        } else {
            ""
        };
        let pri = if mon.is_primary { " (Primary)" } else { "" };
        lines.push(DoctorLineData {
            text: format!(
                "    - {}: {}x{} @ {:.1} Hz{}{}",
                mon.name, mon.width, mon.height, mon.refresh_rate, pri, tag
            )
            .into(),
            color: normal_color,
            bold: mon.is_active_focus,
        });
    }

    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "[10/10] Đo Độ Trễ Phần Cứng & Pacing Đề Xuất:".into(),
        color: header_color,
        bold: true,
    });
    lines.push(DoctorLineData {
        text: format!(
            "  • Uinput RTT: {:.2} ms (Jitter: ±{:.2} ms, Write: {:.1} µs)",
            report.latency.measured_rtt_ms,
            report.latency.measured_jitter_ms,
            report.latency.uinput_write_us
        )
        .into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: format!(
            "  • Bàn phím lặp lại: Delay {} ms, Interval {} ms",
            report.latency.keyboard_repeat_delay_ms, report.latency.keyboard_repeat_interval_ms
        )
        .into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: format!("  • Nguy cơ nhấp nháy: {}", report.pacing.flicker_risk).into(),
        color: if report.pacing.estimated_replace_ms <= 30.0 {
            ok_color
        } else {
            warn_color
        },
        bold: true,
    });
    lines.push(DoctorLineData {
        text: format!("  • Phân tích: {}", report.pacing.explanation).into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "  • Cấu hình Pacing tối ưu:".into(),
        color: header_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: format!(
            "    - Gap: {} ms | Terminal Delay: {} ms",
            report.pacing.gap_ms, report.pacing.terminal_post_delay_ms
        )
        .into(),
        color: ok_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: format!(
            "    - Web Browser: {} ms | DraftJS: {} ms | Heavy App: {} ms",
            report.pacing.browser_post_delay_ms,
            report.pacing.draftjs_post_delay_ms,
            report.pacing.heavy_app_post_delay_ms
        )
        .into(),
        color: ok_color,
        bold: false,
    });

    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "=".repeat(55).into(),
        color: dim_color,
        bold: false,
    });

    if report.issues.is_empty() && report.warnings.is_empty() {
        lines.push(DoctorLineData {
            text: "✔ TẤT CẢ KIỂM TRA ĐỀU HOÀN HẢO! Clak đã sẵn sàng hoạt động tối ưu.".into(),
            color: ok_color,
            bold: true,
        });
    } else {
        if !report.issues.is_empty() {
            lines.push(DoctorLineData {
                text: format!("❌ CẦN XỬ LÝ ({} vấn đề):", report.issues.len()).into(),
                color: error_color,
                bold: true,
            });
            for (i, iss) in report.issues.iter().enumerate() {
                lines.push(DoctorLineData {
                    text: format!("  {}. {}", i + 1, iss.message).into(),
                    color: error_color,
                    bold: false,
                });
                if let Some(ref fix) = iss.fix_command {
                    lines.push(DoctorLineData {
                        text: format!("     Lệnh sửa: {}", fix).into(),
                        color: code_color,
                        bold: false,
                    });
                }
            }
        }
        if !report.warnings.is_empty() {
            lines.push(DoctorLineData {
                text: format!("⚠️  LƯU Ý ({} khuyến nghị):", report.warnings.len()).into(),
                color: warn_color,
                bold: true,
            });
            for (i, warn) in report.warnings.iter().enumerate() {
                lines.push(DoctorLineData {
                    text: format!("  {}. {}", i + 1, warn.message).into(),
                    color: warn_color,
                    bold: false,
                });
                if let Some(ref fix) = warn.fix_command {
                    lines.push(DoctorLineData {
                        text: format!("     Khuyến nghị: {}", fix).into(),
                        color: code_color,
                        bold: false,
                    });
                }
            }
        }
    }
    lines.push(DoctorLineData {
        text: "=".repeat(55).into(),
        color: dim_color,
        bold: false,
    });

    lines
}
