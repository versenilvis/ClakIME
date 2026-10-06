slint::include_modules!();

use clak_diagnostics::DiagnosticReport;
use clak_engine::config::{ClakConfig, MacroItem};
use slint::{ComponentHandle, Model, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// build colorized doctor log lines for slint terminal console
fn format_doctor_lines(report: &DiagnosticReport) -> Vec<DoctorLineData> {
    let mut lines = Vec::new();
    let header_color = slint::Color::from_argb_u8(255, 56, 189, 248);
    let normal_color = slint::Color::from_argb_u8(255, 212, 212, 216);
    let ok_color = slint::Color::from_argb_u8(255, 52, 211, 153);
    let warn_color = slint::Color::from_argb_u8(255, 251, 191, 36);
    let error_color = slint::Color::from_argb_u8(255, 248, 113, 113);
    let code_color = slint::Color::from_argb_u8(255, 167, 139, 250);
    let dim_color = slint::Color::from_argb_u8(255, 113, 113, 122);

    lines.push(DoctorLineData {
        text: "[1/5] Môi trường hệ thống:".into(),
        color: header_color,
        bold: true,
    });
    lines.push(DoctorLineData {
        text: format!("  • Hệ điều hành: {} ({})", report.system.os_name, report.system.architecture).into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: format!("  • Nhân Linux: {}", report.system.kernel_release).into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: format!("  • Session: {} (Desktop: {})", report.system.session_type, report.system.desktop_environment).into(),
        color: normal_color,
        bold: false,
    });
    lines.push(DoctorLineData {
        text: "".into(),
        color: normal_color,
        bold: false,
    });

    lines.push(DoctorLineData {
        text: "[2/5] Thiết bị ảo và Quyền hạn:".into(),
        color: header_color,
        bold: true,
    });
    if report.permissions.uinput_writable {
        lines.push(DoctorLineData {
            text: "  • /dev/uinput: [OK] Khả dụng và có quyền ghi".into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • /dev/uinput: [THIẾU QUYỀN] Người dùng chưa có quyền ghi vào /dev/uinput".into(),
            color: error_color,
            bold: true,
        });
    }
    if report.permissions.input_readable_count > 0 {
        lines.push(DoctorLineData {
            text: format!("  • /dev/input (Mouse Tracking): [OK] Đọc được {}/{} thiết bị", report.permissions.input_readable_count, report.permissions.input_total_count).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • /dev/input (Mouse Tracking): [CẢNH BÁO] Không đọc được thiết bị input".into(),
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
        text: "[3/5] Trạng thái Daemon Fcitx5:".into(),
        color: header_color,
        bold: true,
    });
    if report.daemon.fcitx5_running {
        let pids_str = report.daemon.fcitx5_pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ");
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
        text: "[4/5] Kiểm tra Binary & Nạp bộ nhớ (RAM):".into(),
        color: header_color,
        bold: true,
    });
    if report.memory.clak_loaded {
        let path = report.memory.mapped_path.as_deref().unwrap_or("libclak.so");
        if report.memory.is_deleted_inode {
            lines.push(DoctorLineData {
                text: format!("  • Bộ nhớ: [CẢNH BÁO] Fcitx5 đang giữ inode CŨ trong RAM: {}", path).into(),
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
        text: "[5/5] Cấu hình và Nhật ký (Log):".into(),
        color: header_color,
        bold: true,
    });
    if report.config.config_file_exists {
        lines.push(DoctorLineData {
            text: format!("  • File cấu hình: [OK] {}", report.config.config_path.display()).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: "  • File cấu hình: [MẶC ĐỊNH] Chưa tạo file riêng, đang dùng giá trị mặc định".into(),
            color: normal_color,
            bold: false,
        });
    }
    if report.config.log_file_exists {
        lines.push(DoctorLineData {
            text: format!("  • File log: [OK] {} ({:.1} KB)", report.config.log_path.display(), report.config.log_size_kb.unwrap_or(0.0)).into(),
            color: ok_color,
            bold: false,
        });
    } else {
        lines.push(DoctorLineData {
            text: format!("  • File log: Chưa kích hoạt ghi log debug vào {}", report.config.log_path.display()).into(),
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
            text: "  • Khởi động cùng hệ thống: [CẢNH BÁO] Chưa kích hoạt tự động chạy khi đăng nhập".into(),
            color: warn_color,
            bold: true,
        });
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
                text: format!("  • {} (PID {}): {}", conf.app_name, conf.pid, conf.description).into(),
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

// extract and save configuration from ui state
fn save_config(
    window: &MainWindow,
    apps_model: &Rc<VecModel<SharedString>>,
    macros_model: &Rc<VecModel<MacroItemData>>,
) {
    let method_str = match window.get_method_index() {
        1 => "vni",
        2 => "viqr",
        3 => "teip_vni",
        _ => "telex",
    };
    let charset_str = match window.get_charset_index() {
        1 => "decomposed",
        _ => "unicode",
    };
    let startup_str = match window.get_startup_mode_index() {
        1 => "english",
        _ => "vietnamese",
    };
    let toggle_sc_str = match window.get_toggle_shortcut_index() {
        1 => "alt_z",
        _ => "ctrl_shift",
    };
    let switch_sc_str = match window.get_switch_shortcut_index() {
        1 => "none",
        _ => "ctrl_space",
    };

    let mut saved_apps = Vec::new();
    for i in 0..apps_model.row_count() {
        if let Some(app) = apps_model.row_data(i) {
            saved_apps.push(app.to_string());
        }
    }

    let mut saved_macros = Vec::new();
    for i in 0..macros_model.row_count() {
        if let Some(item) = macros_model.row_data(i) {
            saved_macros.push(MacroItem {
                trigger: item.trigger.to_string(),
                replace: item.replace.to_string(),
            });
        }
    }

    let mut cfg_to_save = ClakConfig::default();
    cfg_to_save.general.method = method_str.to_string();
    cfg_to_save.general.charset = charset_str.to_string();
    cfg_to_save.general.startup_mode = startup_str.to_string();
    cfg_to_save.general.short_w = window.get_short_w();
    cfg_to_save.general.bracket_brackets = window.get_bracket_brackets();

    cfg_to_save.spelling.enabled = window.get_spelling_enabled();
    cfg_to_save.spelling.auto_restore = window.get_auto_restore();
    cfg_to_save.spelling.modern_tone = window.get_modern_tone();

    cfg_to_save.typing.double_space_period = window.get_double_space_period();
    cfg_to_save.typing.auto_capitalize = window.get_auto_capitalize();
    cfg_to_save.macros.enabled = window.get_macros_enabled();
    cfg_to_save.macros.items = saved_macros;

    cfg_to_save.shortcuts.toggle_vietnamese = toggle_sc_str.to_string();
    cfg_to_save.shortcuts.switch_method = switch_sc_str.to_string();

    cfg_to_save.per_app.remember_state = window.get_remember_app_state();
    cfg_to_save.per_app.excluded_apps = saved_apps;

    cfg_to_save.advanced.autostart = window.get_autostart_system();
    cfg_to_save.advanced.uinput_ack = window.get_uinput_ack();
    cfg_to_save.advanced.debug_log = window.get_debug_log();

    cfg_to_save.update.auto_update = window.get_auto_update_enabled();
    cfg_to_save.update.poll_index = window.get_update_poll_index();

    let _ = cfg_to_save.save();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ClakConfig::load();
    let main_window = MainWindow::new()?;

    // populate initial scalar properties
    main_window.set_app_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
    main_window.set_git_commit_hash(env!("GIT_HASH").into());
    main_window.set_git_commit_note(env!("GIT_NOTE").into());
    main_window.set_git_commit_date(env!("GIT_DATE").into());
    main_window.set_auto_update_enabled(cfg.update.auto_update);
    main_window.set_update_poll_index(cfg.update.poll_index);
    let method_idx = match cfg.general.method.as_str() {
        "vni" => 1,
        "viqr" => 2,
        "teip_vni" => 3,
        _ => 0,
    };
    main_window.set_method_index(method_idx);

    let charset_idx = match cfg.general.charset.as_str() {
        "decomposed" => 1,
        _ => 0,
    };
    main_window.set_charset_index(charset_idx);

    let startup_idx = match cfg.general.startup_mode.as_str() {
        "english" => 1,
        _ => 0,
    };
    main_window.set_startup_mode_index(startup_idx);

    main_window.set_short_w(cfg.general.short_w);
    main_window.set_bracket_brackets(cfg.general.bracket_brackets);

    main_window.set_spelling_enabled(cfg.spelling.enabled);
    main_window.set_auto_restore(cfg.spelling.auto_restore);
    main_window.set_modern_tone(cfg.spelling.modern_tone);

    main_window.set_double_space_period(cfg.typing.double_space_period);
    main_window.set_auto_capitalize(cfg.typing.auto_capitalize);
    main_window.set_macros_enabled(cfg.macros.enabled);

    let toggle_sc_idx = match cfg.shortcuts.toggle_vietnamese.as_str() {
        "alt_z" => 1,
        _ => 0,
    };
    main_window.set_toggle_shortcut_index(toggle_sc_idx);

    if let Ok(tab_str) = std::env::var("CLAK_TAB") {
        if let Ok(tab_num) = tab_str.parse::<i32>() {
            main_window.set_selected_tab(tab_num);
        }
    }

    let switch_sc_idx = match cfg.shortcuts.switch_method.as_str() {
        "none" => 1,
        _ => 0,
    };
    main_window.set_switch_shortcut_index(switch_sc_idx);

    main_window.set_autostart_system(cfg.advanced.autostart);
    main_window.set_remember_app_state(cfg.per_app.remember_state);
    main_window.set_uinput_ack(cfg.advanced.uinput_ack);
    main_window.set_debug_log(cfg.advanced.debug_log);

    // populate models
    let apps_vec: Vec<SharedString> = cfg
        .per_app
        .excluded_apps
        .into_iter()
        .map(SharedString::from)
        .collect();
    let apps_model = Rc::new(VecModel::from(apps_vec));
    main_window.set_excluded_apps(apps_model.clone().into());

    let macros_vec: Vec<MacroItemData> = cfg
        .macros
        .items
        .into_iter()
        .map(|m| MacroItemData {
            trigger: SharedString::from(m.trigger),
            replace: SharedString::from(m.replace),
        })
        .collect();
    let macros_model = Rc::new(VecModel::from(macros_vec));
    main_window.set_macro_items(macros_model.clone().into());

    // app exclusion callbacks
    let apps_model_add = apps_model.clone();
    let win_weak_add_app = main_window.as_weak();
    main_window.on_add_excluded_app(move |app_name| {
        let trimmed = app_name.trim();
        if !trimmed.is_empty() {
            apps_model_add.push(SharedString::from(trimmed));
            if let Some(w) = win_weak_add_app.upgrade() {
                w.set_has_changes(true);
            }
        }
    });

    let apps_model_rm = apps_model.clone();
    let win_weak_rm_app = main_window.as_weak();
    main_window.on_remove_excluded_app(move |idx| {
        if idx >= 0 && (idx as usize) < apps_model_rm.row_count() {
            apps_model_rm.remove(idx as usize);
            if let Some(w) = win_weak_rm_app.upgrade() {
                w.set_has_changes(true);
            }
        }
    });

    // macro callbacks
    let macros_model_add = macros_model.clone();
    let win_weak_add_macro = main_window.as_weak();
    main_window.on_add_macro(move |trigger, replace| {
        let t = trigger.trim();
        let r = replace.trim();
        if !t.is_empty() && !r.is_empty() {
            macros_model_add.push(MacroItemData {
                trigger: SharedString::from(t),
                replace: SharedString::from(r),
            });
            if let Some(w) = win_weak_add_macro.upgrade() {
                w.set_has_changes(true);
            }
        }
    });

    let macros_model_rm = macros_model.clone();
    let win_weak_rm_macro = main_window.as_weak();
    main_window.on_remove_macro(move |idx| {
        if idx >= 0 && (idx as usize) < macros_model_rm.row_count() {
            macros_model_rm.remove(idx as usize);
            if let Some(w) = win_weak_rm_macro.upgrade() {
                w.set_has_changes(true);
            }
        }
    });

    // doctor callbacks
    let last_report: Arc<Mutex<Option<DiagnosticReport>>> = Arc::new(Mutex::new(None));
    let last_report_scan = last_report.clone();
    let last_report_copy = last_report.clone();

    let win_scan = main_window.as_weak();
    main_window.on_doctor_scan_requested(move || {
        let Some(window) = win_scan.upgrade() else { return; };
        window.set_doctor_is_scanning(true);

        let win_async = win_scan.clone();
        let last_report_async = last_report_scan.clone();

        std::thread::spawn(move || {
            let report = clak_diagnostics::run_diagnostics();
            let is_healthy = report.is_healthy();
            let now = chrono::Local::now().format("%H:%M:%S · %d/%m/%Y").to_string();
            let lines = format_doctor_lines(&report);

            let summary = if report.issues.is_empty() && report.warnings.is_empty() {
                "Tất cả kiểm tra đều hoàn hảo. Clak sẵn sàng hoạt động tối ưu.".to_string()
            } else {
                format!("Phát hiện {} vấn đề cần xử lý, {} lưu ý.", report.issues.len(), report.warnings.len())
            };

            let report_for_ui = report.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_async.upgrade() {
                    let lines_model = Rc::new(VecModel::from(lines));
                    w.set_doctor_lines(lines_model.into());
                    w.set_doctor_last_scanned(now.into());
                    w.set_doctor_summary_text(summary.into());
                    w.set_doctor_is_healthy(is_healthy);
                    w.set_doctor_has_scanned(true);
                    w.set_doctor_is_scanning(false);
                    if let Ok(mut guard) = last_report_async.lock() {
                        *guard = Some(report_for_ui);
                    }
                }
            });
        });
    });

    let win_copy = main_window.as_weak();
    main_window.on_doctor_copy_requested(move || {
        let Some(window) = win_copy.upgrade() else { return; };
        let md = if let Ok(guard) = last_report_copy.lock() {
            if let Some(ref report) = *guard {
                report.to_markdown()
            } else {
                clak_diagnostics::run_diagnostics().to_markdown()
            }
        } else {
            clak_diagnostics::run_diagnostics().to_markdown()
        };

        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(md);
            window.set_doctor_copied_toast(true);
            let win_timer = win_copy.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(2000), move || {
                if let Some(w) = win_timer.upgrade() {
                    w.set_doctor_copied_toast(false);
                }
            });
        }
    });

    // action callbacks: ok, apply, cancel, esc
    let win_ok = main_window.as_weak();
    let apps_ok = apps_model.clone();
    let macros_ok = macros_model.clone();
    main_window.on_ok_requested(move || {
        let Some(window) = win_ok.upgrade() else {
            return;
        };
        if window.get_has_changes() {
            save_config(&window, &apps_ok, &macros_ok);
        }
        let _ = slint::quit_event_loop();
    });

    let win_apply = main_window.as_weak();
    let apps_apply = apps_model.clone();
    let macros_apply = macros_model.clone();
    main_window.on_apply_requested(move || {
        let Some(window) = win_apply.upgrade() else {
            return;
        };
        save_config(&window, &apps_apply, &macros_apply);
        window.set_has_changes(false);
        window.set_saved_toast(true);
        let win_toast = win_apply.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(2000), move || {
            if let Some(w) = win_toast.upgrade() {
                w.set_saved_toast(false);
            }
        });
    });

    main_window.on_cancel_requested(move || {
        let _ = slint::quit_event_loop();
    });

    let win_esc = main_window.as_weak();
    let apps_esc = apps_model.clone();
    let macros_esc = macros_model.clone();
    main_window.on_escape_requested(move || {
        let Some(window) = win_esc.upgrade() else {
            return;
        };
        if window.get_has_changes() {
            save_config(&window, &apps_esc, &macros_esc);
        }
        let _ = slint::quit_event_loop();
    });

    main_window.on_open_url(|url| {
        let _ = std::process::Command::new("xdg-open")
            .arg(url.as_str())
            .spawn();
    });

    let win_reset = main_window.as_weak();
    let apps_reset = apps_model.clone();
    let macros_reset = macros_model.clone();
    main_window.on_reset_defaults_requested(move || {
        let Some(window) = win_reset.upgrade() else {
            return;
        };
        let default_cfg = ClakConfig::default();
        window.set_method_index(0);
        window.set_charset_index(0);
        window.set_startup_mode_index(0);
        window.set_short_w(default_cfg.general.short_w);
        window.set_bracket_brackets(default_cfg.general.bracket_brackets);
        window.set_spelling_enabled(default_cfg.spelling.enabled);
        window.set_auto_restore(default_cfg.spelling.auto_restore);
        window.set_modern_tone(default_cfg.spelling.modern_tone);
        window.set_double_space_period(default_cfg.typing.double_space_period);
        window.set_auto_capitalize(default_cfg.typing.auto_capitalize);
        window.set_macros_enabled(default_cfg.macros.enabled);
        window.set_toggle_shortcut_index(0);
        window.set_switch_shortcut_index(0);
        window.set_autostart_system(default_cfg.advanced.autostart);
        window.set_remember_app_state(default_cfg.per_app.remember_state);
        window.set_uinput_ack(default_cfg.advanced.uinput_ack);
        window.set_debug_log(default_cfg.advanced.debug_log);
        window.set_auto_update_enabled(default_cfg.update.auto_update);
        window.set_update_poll_index(default_cfg.update.poll_index);

        while macros_reset.row_count() > 0 {
            macros_reset.remove(0);
        }
        for m in default_cfg.macros.items {
            macros_reset.push(MacroItemData {
                trigger: m.trigger.into(),
                replace: m.replace.into(),
            });
        }

        while apps_reset.row_count() > 0 {
            apps_reset.remove(0);
        }
        for a in default_cfg.per_app.excluded_apps {
            apps_reset.push(a.into());
        }

        window.set_has_changes(true);
    });

    let win_update = main_window.as_weak();
    main_window.on_check_update_requested(move || {
        let Some(window) = win_update.upgrade() else {
            return;
        };
        window.set_is_updating(true);
        window.set_update_status_text("Đang kiểm tra và cập nhật...".into());

        let win_weak_thread = win_update.clone();
        std::thread::spawn(move || {
            let terminals = [
                "kitty",
                "alacritty",
                "foot",
                "wezterm",
                "gnome-terminal",
                "konsole",
                "xterm",
            ];
            let term = terminals.iter().find(|&&t| {
                std::process::Command::new("which")
                    .arg(t)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            });

            let script_path = if std::path::Path::new("scripts/update.sh").exists() {
                std::path::PathBuf::from("scripts/update.sh")
            } else if let Ok(home) = std::env::var("HOME") {
                std::path::PathBuf::from(home).join("dev/github/input-method/scripts/update.sh")
            } else {
                std::path::PathBuf::from("/tmp/update.sh")
            };

            let res = if let Some(&term_cmd) = term {
                let cmd_str = format!(
                    "bash \"{}\"; echo ''; read -p 'Nhấn Enter để đóng...' dummy",
                    script_path.display()
                );
                std::process::Command::new(term_cmd)
                    .args(["-e", "bash", "-c", &cmd_str])
                    .status()
            } else {
                std::process::Command::new("bash")
                    .arg(&script_path)
                    .status()
            };

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_weak_thread.upgrade() {
                    w.set_is_updating(false);
                    match res {
                        Ok(s) if s.success() => {
                            w.set_update_status_text("Cập nhật hoàn tất!".into());
                        }
                        _ => {
                            w.set_update_status_text("Quá trình cập nhật đã kết thúc".into());
                        }
                    }
                }
            });
        });
    });

    let win_uninstall = main_window.as_weak();
    main_window.on_uninstall_requested(move || {
        let terminals = [
            "kitty",
            "alacritty",
            "foot",
            "wezterm",
            "gnome-terminal",
            "konsole",
            "xterm",
        ];
        let term = terminals.iter().find(|&&t| {
            std::process::Command::new("which")
                .arg(t)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        });

        let script_cmd = "if [ -f scripts/uninstall.sh ]; then bash scripts/uninstall.sh; elif [ -f \"$HOME/.local/share/clak/uninstall.sh\" ]; then bash \"$HOME/.local/share/clak/uninstall.sh\"; else curl -fsSL https://raw.githubusercontent.com/versenilvis/clak/main/scripts/uninstall.sh | bash; fi; echo ''; read -p 'Nhấn Enter để đóng...' dummy";

        if let Some(&term_cmd) = term {
            let mut cmd = std::process::Command::new(term_cmd);
            match term_cmd {
                "gnome-terminal" => {
                    cmd.args(["--", "bash", "-c", script_cmd]);
                }
                "wezterm" => {
                    cmd.args(["start", "--", "bash", "-c", script_cmd]);
                }
                _ => {
                    cmd.args(["-e", "bash", "-c", script_cmd]);
                }
            }
            let _ = cmd.spawn();
        } else {
            let _ = std::process::Command::new("bash")
                .args(["-c", script_cmd])
                .spawn();
        }

        if let Some(w) = win_uninstall.upgrade() {
            let _ = w.hide();
        }
        std::process::exit(0);
    });

    main_window.run()?;
    Ok(())
}
