slint::include_modules!();

mod config_sync;
mod doctor_formatter;
mod shortcuts;
mod updater;

use clak_diagnostics::DiagnosticReport;
use clak_engine::config::ClakConfig;
use slint::{ComponentHandle, Model, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use config_sync::save_config;
use doctor_formatter::format_doctor_lines;
use shortcuts::{format_recorded_shortcut, shortcut_id_to_display};
use updater::{check_can_rollback, fetch_available_versions};

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

    let (is_pacman, can_rb, rb_text, prev_backup_ver) = check_can_rollback();
    main_window.set_is_pacman(is_pacman);
    main_window.set_can_rollback(can_rb);
    main_window.set_rollback_version_text(rb_text.into());

    let home_dir = std::env::var("HOME").unwrap_or_default();
    let version_items = fetch_available_versions(&home_dir, is_pacman, prev_backup_ver.as_deref());
    let slint_versions: Vec<slint::SharedString> = version_items
        .iter()
        .map(|item| item.label.as_str().into())
        .collect();
    let model = std::rc::Rc::new(slint::VecModel::from(slint_versions));
    main_window.set_available_versions(model.clone().into());
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

    main_window.set_spelling_enabled(cfg.spelling.auto_restore);
    main_window.set_auto_restore(cfg.spelling.auto_restore);
    main_window.set_auto_restore_english(cfg.spelling.auto_restore_english);
    main_window.set_allow_foreign_consonants(cfg.spelling.allow_foreign_consonants);
    main_window.set_modern_tone(cfg.spelling.modern_tone);
    main_window.set_autocorrect(cfg.spelling.autocorrect);

    main_window.set_double_space_period(cfg.typing.double_space_period);
    main_window.set_auto_capitalize(cfg.typing.auto_capitalize);
    main_window.set_shift_autofix(cfg.typing.shift_autofix);
    main_window.set_macros_enabled(cfg.macros.enabled);

    let toggle_sc_display = shortcut_id_to_display(&cfg.shortcuts.toggle_vietnamese);
    main_window.set_toggle_shortcut_text(toggle_sc_display.into());

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

    main_window.set_adaptive_pacing(cfg.pacing.adaptive);
    let pacing_idx = match cfg.pacing.preset.as_str() {
        "fast" => 0,
        "balanced" => 1,
        "safe" => 2,
        "custom" => 3,
        _ => 3,
    };
    main_window.set_pacing_preset_index(pacing_idx);
    main_window.set_draftjs_delay_ms(cfg.pacing.draftjs_delay_ms as i32);
    main_window.set_draftjs_gap_ms(cfg.pacing.draftjs_gap_ms as i32);
    main_window.set_terminal_delay_ms(cfg.pacing.terminal_delay_ms as i32);
    main_window.set_terminal_gap_ms(cfg.pacing.terminal_gap_ms as i32);
    main_window.set_heavy_app_delay_ms(cfg.pacing.heavy_app_delay_ms as i32);
    main_window.set_heavy_app_gap_ms(cfg.pacing.heavy_app_gap_ms as i32);

    // populate models
    let mut rules_vec: Vec<AppRuleData> = Vec::new();
    for (k, v) in &cfg.per_app.rules {
        rules_vec.push(AppRuleData {
            name: SharedString::from(k),
            mode: SharedString::from(v),
        });
    }
    for a in &cfg.per_app.excluded_apps {
        if !cfg.per_app.rules.contains_key(a) {
            rules_vec.push(AppRuleData {
                name: SharedString::from(a),
                mode: SharedString::from("off"),
            });
        }
    }
    rules_vec.sort_by(|a, b| a.name.cmp(&b.name));
    let apps_model = Rc::new(VecModel::from(rules_vec));
    main_window.set_app_rules(apps_model.clone().into());

    fn is_colon_dynamic(t: &str) -> bool {
        t.len() >= 3 && t.starts_with(':') && t.ends_with(':')
    }
    fn build_macro_items(items: &[clak_engine::config::MacroItem]) -> Vec<MacroItemData> {
        let mut vec: Vec<MacroItemData> = items
            .iter()
            .map(|m| {
                let dyn_flag = is_colon_dynamic(m.trigger.trim());
                MacroItemData {
                    trigger: SharedString::from(&m.trigger),
                    replace: SharedString::from(&m.replace),
                    tab_only: m.tab_only || dyn_flag,
                    is_dynamic: dyn_flag,
                    is_builtin: false,
                }
            })
            .collect();
        for (trig, desc) in clak_engine::ime::dynamic_macro::builtin_dynamic_macro_docs() {
            vec.push(MacroItemData {
                trigger: SharedString::from(*trig),
                replace: SharedString::from(*desc),
                tab_only: true,
                is_dynamic: true,
                is_builtin: true,
            });
        }
        vec
    }

    let macros_vec = build_macro_items(&cfg.macros.items);
    let macros_model = Rc::new(VecModel::from(macros_vec));
    main_window.set_macro_items(macros_model.clone().into());

    // app rule callbacks
    let apps_model_add = apps_model.clone();
    let win_weak_add_app = main_window.as_weak();
    main_window.on_add_app_rule(move |app_name, mode_str| {
        let trimmed = app_name.trim();
        if !trimmed.is_empty() {
            let mut found = false;
            for i in 0..apps_model_add.row_count() {
                if let Some(mut item) = apps_model_add.row_data(i) {
                    if item.name.to_lowercase() == trimmed.to_lowercase() {
                        item.mode = mode_str.clone();
                        apps_model_add.set_row_data(i, item);
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                apps_model_add.push(AppRuleData {
                    name: SharedString::from(trimmed),
                    mode: mode_str,
                });
            }
            if let Some(w) = win_weak_add_app.upgrade() {
                w.set_has_changes(true);
            }
        }
    });

    let apps_model_rm = apps_model.clone();
    let win_weak_rm_app = main_window.as_weak();
    main_window.on_remove_app_rule(move |idx| {
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
    main_window.on_add_macro(move |trigger, replace, tab_only| {
        let t = trigger.trim();
        let r = replace.trim();
        if !t.is_empty() && !r.is_empty() {
            let dyn_flag = is_colon_dynamic(t);
            macros_model_add.insert(
                0,
                MacroItemData {
                    trigger: SharedString::from(t),
                    replace: SharedString::from(r),
                    tab_only: tab_only || dyn_flag,
                    is_dynamic: dyn_flag,
                    is_builtin: false,
                },
            );
            if let Some(w) = win_weak_add_macro.upgrade() {
                w.set_has_changes(true);
            }
        }
    });

    let macros_model_toggle = macros_model.clone();
    let win_weak_toggle_macro = main_window.as_weak();
    main_window.on_toggle_macro_tab(move |idx| {
        if idx >= 0 && (idx as usize) < macros_model_toggle.row_count() {
            if let Some(mut item) = macros_model_toggle.row_data(idx as usize) {
                if !item.is_dynamic && !item.is_builtin {
                    item.tab_only = !item.tab_only;
                    macros_model_toggle.set_row_data(idx as usize, item);
                    if let Some(w) = win_weak_toggle_macro.upgrade() {
                        w.set_has_changes(true);
                    }
                }
            }
        }
    });

    let macros_model_rm = macros_model.clone();
    let win_weak_rm_macro = main_window.as_weak();
    main_window.on_remove_macro(move |idx| {
        if idx >= 0 && (idx as usize) < macros_model_rm.row_count() {
            if let Some(item) = macros_model_rm.row_data(idx as usize) {
                if !item.is_builtin {
                    macros_model_rm.remove(idx as usize);
                    if let Some(w) = win_weak_rm_macro.upgrade() {
                        w.set_has_changes(true);
                    }
                }
            }
        }
    });

    // doctor callbacks
    let last_report: Arc<Mutex<Option<DiagnosticReport>>> = Arc::new(Mutex::new(None));
    let last_report_scan = last_report.clone();
    let last_report_copy = last_report.clone();

    let win_scan = main_window.as_weak();
    main_window.on_doctor_scan_requested(move || {
        let Some(window) = win_scan.upgrade() else {
            return;
        };
        window.set_doctor_is_scanning(true);

        let win_async = win_scan.clone();
        let last_report_async = last_report_scan.clone();

        std::thread::spawn(move || {
            let report = clak_diagnostics::run_diagnostics();
            let is_healthy = report.is_healthy();
            let time_tag = chrono::Local::now().format("%H:%M:%S").to_string();
            let now = chrono::Local::now()
                .format("%H:%M:%S · %d/%m/%Y")
                .to_string();
            let lines = format_doctor_lines(&report, &time_tag);

            let mut score: i32 = 100;
            score -= (report.issues.len() as i32) * 25;
            score -= (report.warnings.len() as i32) * 10;
            let score = score.clamp(0, 100);

            let (score_level, score_label, score_color) = if score >= 90 {
                (
                    0,
                    "Khỏe mạnh",
                    slint::Color::from_argb_u8(255, 16, 185, 129),
                )
            } else if score >= 60 {
                (
                    1,
                    "Cần lưu ý",
                    slint::Color::from_argb_u8(255, 245, 158, 11),
                )
            } else {
                (2, "Nguy hiểm", slint::Color::from_argb_u8(255, 239, 68, 68))
            };

            let summary = if report.issues.is_empty() && report.warnings.is_empty() {
                "Tất cả kiểm tra đều hoàn hảo. Clak sẵn sàng hoạt động tối ưu.".to_string()
            } else {
                format!(
                    "Phát hiện {} vấn đề cần xử lý, {} lưu ý.",
                    report.issues.len(),
                    report.warnings.len()
                )
            };

            let report_for_ui = report.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_async.upgrade() {
                    let lines_model = Rc::new(VecModel::from(lines));
                    w.set_doctor_lines(lines_model.into());
                    w.set_doctor_last_scanned(now.into());
                    w.set_doctor_summary_text(summary.into());
                    w.set_doctor_is_healthy(is_healthy);
                    w.set_doctor_score_level(score_level);
                    w.set_doctor_score_text(score.to_string().into());
                    w.set_doctor_score_label(score_label.into());
                    w.set_doctor_score_color(score_color);
                    w.set_doctor_has_scanned(true);
                    w.set_doctor_is_scanning(false);
                    let is_deleted = report_for_ui.memory.is_deleted_inode;
                    let addon_not_loaded =
                        report_for_ui.daemon.fcitx5_running && !report_for_ui.memory.clak_loaded;
                    let profile_needs = !report_for_ui.profile.clak_in_profile
                        || !report_for_ui.profile.is_default_im;
                    let autostart_needs = !report_for_ui.autostart.is_enabled;
                    let permissions_needs = !report_for_ui.permissions.uinput_writable
                        || report_for_ui.permissions.input_readable_count == 0;
                    let wps_fix = report_for_ui.wps.installed && !report_for_ui.wps.configured;
                    let jb_fix = report_for_ui.jetbrains.installed
                        && !report_for_ui.jetbrains.unconfigured_ides.is_empty();

                    let current_im_name = report_for_ui
                        .daemon
                        .current_im
                        .clone()
                        .unwrap_or_else(|| "Không rõ".to_string());
                    let im_switch = report_for_ui.daemon.fcitx5_running
                        && report_for_ui.daemon.current_im.as_deref() != Some("clak");

                    let cur_cfg = clak_engine::config::ClakConfig::load();
                    let pacing_differs = cur_cfg.pacing.preset != "custom"
                        || cur_cfg.pacing.draftjs_delay_ms != report_for_ui.pacing.draftjs_post_delay_ms
                        || cur_cfg.pacing.terminal_delay_ms != report_for_ui.pacing.terminal_post_delay_ms
                        || cur_cfg.pacing.draftjs_gap_ms != report_for_ui.pacing.gap_ms;

                    let pacing_tune_desc = format!(
                        "Màn hình {:.0}Hz (budget {:.1}ms). Đề xuất: Terminal {}ms, Browser {}ms, DraftJS {}ms (Gap {}ms)",
                        report_for_ui.display.effective_refresh_rate,
                        report_for_ui.display.frame_budget_ms,
                        report_for_ui.pacing.terminal_post_delay_ms,
                        report_for_ui.pacing.browser_post_delay_ms,
                        report_for_ui.pacing.draftjs_post_delay_ms,
                        report_for_ui.pacing.gap_ms
                    );

                    let env_conflict = report_for_ui.environment.gtk_im_module.as_deref() == Some("ibus")
                        || report_for_ui.environment.qt_im_module.as_deref() == Some("ibus")
                        || report_for_ui
                            .environment
                            .xmodifiers
                            .as_deref()
                            .is_some_and(|x| x.contains("ibus"))
                        || !report_for_ui.environment.env_file_exists;

                    let has_fixable = !report_for_ui.is_healthy()
                        || is_deleted
                        || addon_not_loaded
                        || profile_needs
                        || autostart_needs
                        || permissions_needs
                        || wps_fix
                        || jb_fix
                        || im_switch
                        || pacing_differs
                        || env_conflict;

                    w.set_reload_needed(is_deleted);
                    w.set_addon_needs_load(addon_not_loaded && !is_deleted);
                    w.set_profile_needs_setup(profile_needs);
                    w.set_env_needs_fix(env_conflict);
                    w.set_autostart_needs_fix(autostart_needs);
                    w.set_permissions_needs_fix(permissions_needs);
                    w.set_wps_needs_fix(wps_fix);
                    w.set_jetbrains_needs_fix(jb_fix);
                    w.set_im_switch_needed(im_switch);
                    w.set_current_im_name(current_im_name.into());
                    w.set_pacing_needs_tune(pacing_differs);
                    w.set_pacing_tune_description(pacing_tune_desc.into());
                    w.set_has_fixable_issues(has_fixable);
                    if let Ok(mut guard) = last_report_async.lock() {
                        *guard = Some(report_for_ui);
                    }
                }
            });
        });
    });

    let win_copy = main_window.as_weak();
    main_window.on_doctor_copy_requested(move || {
        let Some(window) = win_copy.upgrade() else {
            return;
        };
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

    let win_record = main_window.as_weak();
    let apps_rec = apps_model.clone();
    let macros_rec = macros_model.clone();
    main_window.on_record_shortcut_pressed(move |text, ctrl, alt, shift, meta| {
        let Some(window) = win_record.upgrade() else {
            return;
        };
        if let Some((display_str, _id_str)) =
            format_recorded_shortcut(text.as_str(), ctrl, alt, shift, meta)
        {
            window.set_toggle_shortcut_text(display_str.into());
            window.set_is_recording_shortcut(false);
            window.set_has_changes(true);
            save_config(&window, &apps_rec, &macros_rec);
        }
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
        window.set_spelling_enabled(default_cfg.spelling.auto_restore);
        window.set_auto_restore(default_cfg.spelling.auto_restore);
        window.set_auto_restore_english(default_cfg.spelling.auto_restore_english);
        window.set_allow_foreign_consonants(default_cfg.spelling.allow_foreign_consonants);
        window.set_modern_tone(default_cfg.spelling.modern_tone);
        window.set_autocorrect(default_cfg.spelling.autocorrect);
        window.set_double_space_period(default_cfg.typing.double_space_period);
        window.set_auto_capitalize(default_cfg.typing.auto_capitalize);
        window.set_shift_autofix(default_cfg.typing.shift_autofix);
        window.set_macros_enabled(default_cfg.macros.enabled);
        window.set_toggle_shortcut_text("Ctrl + Shift".into());
        window.set_is_recording_shortcut(false);
        window.set_switch_shortcut_index(0);
        window.set_autostart_system(default_cfg.advanced.autostart);
        window.set_remember_app_state(default_cfg.per_app.remember_state);
        window.set_uinput_ack(default_cfg.advanced.uinput_ack);
        window.set_debug_log(default_cfg.advanced.debug_log);

        window.set_adaptive_pacing(default_cfg.pacing.adaptive);
        let default_pacing_idx = match default_cfg.pacing.preset.as_str() {
            "fast" => 0,
            "balanced" => 1,
            "safe" => 2,
            "custom" => 3,
            _ => 3,
        };
        window.set_pacing_preset_index(default_pacing_idx);
        window.set_draftjs_delay_ms(default_cfg.pacing.draftjs_delay_ms as i32);
        window.set_draftjs_gap_ms(default_cfg.pacing.draftjs_gap_ms as i32);
        window.set_terminal_delay_ms(default_cfg.pacing.terminal_delay_ms as i32);
        window.set_terminal_gap_ms(default_cfg.pacing.terminal_gap_ms as i32);
        window.set_heavy_app_delay_ms(default_cfg.pacing.heavy_app_delay_ms as i32);
        window.set_heavy_app_gap_ms(default_cfg.pacing.heavy_app_gap_ms as i32);

        window.set_auto_update_enabled(default_cfg.update.auto_update);
        window.set_update_poll_index(default_cfg.update.poll_index);

        while macros_reset.row_count() > 0 {
            macros_reset.remove(0);
        }
        for item in build_macro_items(&default_cfg.macros.items) {
            macros_reset.push(item);
        }

        while apps_reset.row_count() > 0 {
            apps_reset.remove(0);
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
                "ptyxis",
                "kgx",
                "gnome-terminal",
                "kitty",
                "alacritty",
                "foot",
                "wezterm",
                "konsole",
                "xfce4-terminal",
                "x-terminal-emulator",
                "xterm",
            ];
            let term = terminals.iter().find(|&&t| {
                std::process::Command::new("which")
                    .arg(t)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            });

            let script_cmd = "if [ -f \"$HOME/.local/share/clak/update.sh\" ]; then bash \"$HOME/.local/share/clak/update.sh\"; else curl -fsSL https://raw.githubusercontent.com/versenilvis/ClakIME/main/scripts/update.sh | bash; fi; echo ''; read -p 'Nhấn Enter để đóng...' dummy";

            let res = if let Some(&term_cmd) = term {
                let mut cmd = std::process::Command::new(term_cmd);
                match term_cmd {
                    "gnome-terminal" | "kgx" | "ptyxis" | "kitty" => {
                        cmd.args(["--", "bash", "-c", script_cmd]);
                    }
                    "foot" => {
                        cmd.args(["bash", "-c", script_cmd]);
                    }
                    "wezterm" => {
                        cmd.args(["start", "--", "bash", "-c", script_cmd]);
                    }
                    _ => {
                        cmd.args(["-e", "bash", "-c", script_cmd]);
                    }
                }
                cmd.status()
            } else {
                std::process::Command::new("bash")
                    .args(["-c", script_cmd])
                    .status()
            };

            let daemon = clak_diagnostics::check_daemon();
            let is_deleted = if daemon.fcitx5_running {
                let memory = clak_diagnostics::check_memory(&daemon.fcitx5_pids);
                memory.is_deleted_inode
            } else {
                false
            };

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_weak_thread.upgrade() {
                    w.set_is_updating(false);
                    w.set_reload_needed(is_deleted);
                    let (_, can_rb, rb_text, _) = check_can_rollback();
                    w.set_can_rollback(can_rb);
                    w.set_rollback_version_text(rb_text.into());
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

    let win_rollback = main_window.as_weak();
    main_window.on_rollback_requested(move || {
        let Some(window) = win_rollback.upgrade() else {
            return;
        };
        window.set_is_updating(true);
        window.set_update_status_text("Đang thực hiện rollback phiên bản...".into());

        let win_weak_thread = win_rollback.clone();
        std::thread::spawn(move || {
            let terminals = [
                "ptyxis",
                "kgx",
                "gnome-terminal",
                "kitty",
                "alacritty",
                "foot",
                "wezterm",
                "konsole",
                "xfce4-terminal",
                "x-terminal-emulator",
                "xterm",
            ];
            let term = terminals.iter().find(|&&t| {
                std::process::Command::new("which")
                    .arg(t)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            });

            let script_cmd = "if [ -f \"$HOME/.local/share/clak/update.sh\" ]; then bash \"$HOME/.local/share/clak/update.sh\" --rollback; else curl -fsSL https://raw.githubusercontent.com/versenilvis/ClakIME/main/scripts/update.sh | bash -s -- --rollback; fi; echo ''; read -p 'Nhấn Enter để đóng...' dummy";

            let res = if let Some(&term_cmd) = term {
                let mut cmd = std::process::Command::new(term_cmd);
                match term_cmd {
                    "gnome-terminal" | "kgx" | "ptyxis" | "kitty" => {
                        cmd.args(["--", "bash", "-c", script_cmd]);
                    }
                    "foot" => {
                        cmd.args(["bash", "-c", script_cmd]);
                    }
                    "wezterm" => {
                        cmd.args(["start", "--", "bash", "-c", script_cmd]);
                    }
                    _ => {
                        cmd.args(["-e", "bash", "-c", script_cmd]);
                    }
                }
                cmd.status()
            } else {
                std::process::Command::new("bash")
                    .args(["-c", script_cmd])
                    .status()
            };

            let daemon = clak_diagnostics::check_daemon();
            let is_deleted = if daemon.fcitx5_running {
                let memory = clak_diagnostics::check_memory(&daemon.fcitx5_pids);
                memory.is_deleted_inode
            } else {
                false
            };

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_weak_thread.upgrade() {
                    w.set_is_updating(false);
                    w.set_reload_needed(is_deleted);
                    let (_, can_rb, rb_text, _) = check_can_rollback();
                    w.set_can_rollback(can_rb);
                    w.set_rollback_version_text(rb_text.into());
                    match res {
                        Ok(s) if s.success() => {
                            w.set_update_status_text("Đã hoàn tất rollback!".into());
                        }
                        _ => {
                            w.set_update_status_text("Quá trình rollback đã kết thúc".into());
                        }
                    }
                }
            });
        });
    });

    let win_install_ver = main_window.as_weak();
    let items_clone = version_items.clone();
    main_window.on_install_version_requested(move |idx| {
        let Some(window) = win_install_ver.upgrade() else {
            return;
        };
        let idx = idx as usize;
        let Some(item) = items_clone.get(idx).cloned() else {
            return;
        };

        window.set_is_updating(true);
        window.set_update_status_text(format!("Đang cài đặt {}...", item.label).into());

        let win_weak_thread = win_install_ver.clone();
        std::thread::spawn(move || {
            let terminals = [
                "ptyxis", "kgx", "gnome-terminal", "kitty", "alacritty",
                "foot", "wezterm", "konsole", "xfce4-terminal",
                "x-terminal-emulator", "xterm",
            ];
            let term = terminals.iter().find(|&&t| {
                std::process::Command::new("which")
                    .arg(t)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            });

            let script_cmd = if item.is_pacman_pkg {
                format!(
                    "echo 'Hạ cấp gói pacman từ cache: {0}'; sudo pacman -U \"{1}\"; echo ''; read -p 'Nhấn Enter để đóng...' dummy",
                    item.label, item.target
                )
            } else if item.is_backup {
                "if [ -f \"$HOME/.local/share/clak/update.sh\" ]; then bash \"$HOME/.local/share/clak/update.sh\" --rollback; else curl -fsSL https://raw.githubusercontent.com/versenilvis/ClakIME/main/scripts/update.sh | bash -s -- --rollback; fi; echo ''; read -p 'Nhấn Enter để đóng...' dummy".to_string()
            } else {
                format!(
                    "if [ -f \"$HOME/.local/share/clak/update.sh\" ]; then bash \"$HOME/.local/share/clak/update.sh\" --version={0}; else curl -fsSL https://raw.githubusercontent.com/versenilvis/ClakIME/main/scripts/update.sh | bash -s -- --version={0}; fi; echo ''; read -p 'Nhấn Enter để đóng...' dummy",
                    item.target
                )
            };

            let res = if let Some(&term_cmd) = term {
                let mut cmd = std::process::Command::new(term_cmd);
                match term_cmd {
                    "gnome-terminal" | "kgx" | "ptyxis" | "kitty" => {
                        cmd.args(["--", "bash", "-c", &script_cmd]);
                    }
                    "foot" => {
                        cmd.args(["bash", "-c", &script_cmd]);
                    }
                    "wezterm" => {
                        cmd.args(["start", "--", "bash", "-c", &script_cmd]);
                    }
                    _ => {
                        cmd.args(["-e", "bash", "-c", &script_cmd]);
                    }
                }
                cmd.status()
            } else {
                std::process::Command::new("bash")
                    .args(["-c", &script_cmd])
                    .status()
            };

            let daemon = clak_diagnostics::check_daemon();
            let is_deleted = if daemon.fcitx5_running {
                let memory = clak_diagnostics::check_memory(&daemon.fcitx5_pids);
                memory.is_deleted_inode
            } else {
                false
            };

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_weak_thread.upgrade() {
                    w.set_is_updating(false);
                    w.set_reload_needed(is_deleted);
                    match res {
                        Ok(s) if s.success() => {
                            w.set_update_status_text(format!("Đã chuyển sang {}!", item.label).into());
                        }
                        _ => {
                            w.set_update_status_text("Quá trình cài đặt đã kết thúc".into());
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

        let script_cmd = "if [ -f \"$HOME/.local/share/clak/uninstall.sh\" ]; then bash \"$HOME/.local/share/clak/uninstall.sh\"; else curl -fsSL https://raw.githubusercontent.com/versenilvis/ClakIME/main/scripts/uninstall.sh | bash; fi; echo ''; read -p 'Nhấn Enter để đóng...' dummy";

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

    let win_restart = main_window.as_weak();
    main_window.on_restart_fcitx5_requested(move || {
        let Some(window) = win_restart.upgrade() else {
            return;
        };
        window.set_is_reloading(true);
        window.set_reload_status_text("Đang khởi động lại Fcitx5...".into());

        let win_async = win_restart.clone();
        std::thread::spawn(move || {
            // Dam bao symlink Ubuntu va cau hinh profile duoc dong bo
            let _ = clak_diagnostics::fix_fcitx5_profile();

            let restarted = clak_diagnostics::restart_fcitx5_daemon();

            // poll fcitx5 daemon readiness instead of blind fixed sleep
            let mut is_deleted = true;
            let mut daemon_ready = false;
            for _ in 0..15 {
                std::thread::sleep(std::time::Duration::from_millis(200));
                let daemon = clak_diagnostics::check_daemon();
                if daemon.fcitx5_running && !daemon.fcitx5_pids.is_empty() {
                    let memory = clak_diagnostics::check_memory(&daemon.fcitx5_pids);
                    if memory.clak_loaded && !memory.is_deleted_inode {
                        is_deleted = false;
                        daemon_ready = true;
                        break;
                    }
                }
            }

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_async.upgrade() {
                    w.set_is_reloading(false);
                    if restarted && daemon_ready && !is_deleted {
                        w.set_reload_needed(false);
                        w.set_addon_needs_load(false);
                        w.set_profile_needs_setup(false);
                        w.set_reload_status_text(
                            "Khởi động lại Fcitx5 thành công! Phiên bản mới đã được nạp.".into(),
                        );
                        if w.get_doctor_has_scanned() {
                            w.invoke_doctor_scan_requested();
                        }
                    } else {
                        w.set_reload_status_text(
                            "Khởi động lại Fcitx5 thất bại hoặc daemon chưa sẵn sàng".into(),
                        );
                    }
                }
            });
        });
    });

    let win_prof = main_window.as_weak();
    main_window.on_setup_profile_requested(move || {
        let Some(window) = win_prof.upgrade() else {
            return;
        };
        window.set_is_reloading(true);
        window.set_reload_status_text("Đang kích hoạt Clak và khởi động lại Fcitx5...".into());
        let win_async = win_prof.clone();
        std::thread::spawn(move || {
            if let Err(e) = clak_diagnostics::fix_fcitx5_profile() {
                eprintln!("lỗi thêm clak vào profile: {}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_async.upgrade() {
                    w.set_is_reloading(false);
                    w.set_profile_needs_setup(false);
                    w.set_addon_needs_load(false);
                    if w.get_doctor_has_scanned() {
                        w.invoke_doctor_scan_requested();
                    }
                }
            });
        });
    });

    let win_wps = main_window.as_weak();
    main_window.on_fix_wps_requested(move || {
        let Some(window) = win_wps.upgrade() else {
            return;
        };
        if let Err(e) = clak_diagnostics::fix_wps_compatibility() {
            eprintln!("lỗi tối ưu wps office: {}", e);
        } else {
            window.set_wps_needs_fix(false);
            if window.get_doctor_has_scanned() {
                window.invoke_doctor_scan_requested();
            }
        }
    });

    let win_jb = main_window.as_weak();
    main_window.on_fix_jetbrains_requested(move || {
        let Some(window) = win_jb.upgrade() else {
            return;
        };
        if let Err(e) = clak_diagnostics::fix_jetbrains_compatibility() {
            eprintln!("lỗi tối ưu jetbrains: {}", e);
        } else {
            window.set_jetbrains_needs_fix(false);
            if window.get_doctor_has_scanned() {
                window.invoke_doctor_scan_requested();
            }
        }
    });

    let win_autofix = main_window.as_weak();
    main_window.on_auto_fix_requested(move || {
        let Some(window) = win_autofix.upgrade() else {
            return;
        };
        window.set_is_fixing(true);
        let win_async = win_autofix.clone();
        std::thread::spawn(move || {
            let _ = clak_diagnostics::auto_fix_all(true);
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_async.upgrade() {
                    w.set_is_fixing(false);
                    w.invoke_doctor_scan_requested();
                }
            });
        });
    });

    let win_env = main_window.as_weak();
    main_window.on_fix_env_requested(move || {
        let Some(window) = win_env.upgrade() else {
            return;
        };
        window.set_is_fixing(true);
        let win_async = win_env.clone();
        std::thread::spawn(move || {
            if let Err(e) = clak_diagnostics::fix_environment_conflicts() {
                eprintln!("lỗi sửa xung đột môi trường: {}", e);
            }
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_async.upgrade() {
                    w.set_env_needs_fix(false);
                    w.set_is_fixing(false);
                    w.invoke_doctor_scan_requested();
                }
            });
        });
    });

    let win_auto = main_window.as_weak();
    main_window.on_fix_autostart_requested(move || {
        let Some(window) = win_auto.upgrade() else {
            return;
        };
        if let Err(e) = clak_diagnostics::enable_autostart() {
            eprintln!("lỗi bật autostart: {}", e);
        } else {
            window.set_autostart_needs_fix(false);
            if window.get_doctor_has_scanned() {
                window.invoke_doctor_scan_requested();
            }
        }
    });

    let win_perm = main_window.as_weak();
    main_window.on_fix_permissions_requested(move || {
        let Some(window) = win_perm.upgrade() else {
            return;
        };
        window.set_is_fixing(true);
        let win_async = win_perm.clone();
        std::thread::spawn(move || {
            if let Err(e) = clak_diagnostics::fix_input_permissions() {
                eprintln!("lỗi phân quyền: {}", e);
            }
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_async.upgrade() {
                    w.set_is_fixing(false);
                    w.invoke_doctor_scan_requested();
                }
            });
        });
    });

    let win_im = main_window.as_weak();
    main_window.on_switch_to_clak_requested(move || {
        let Some(window) = win_im.upgrade() else {
            return;
        };
        clak_diagnostics::switch_to_clak();
        window.set_im_switch_needed(false);
        window.set_saved_toast(true);
        let win_toast = win_im.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(1500), move || {
            if let Some(w) = win_toast.upgrade() {
                w.set_saved_toast(false);
            }
        });
        let win_async = win_im.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_async.upgrade() {
                    if w.get_doctor_has_scanned() {
                        w.invoke_doctor_scan_requested();
                    }
                }
            });
        });
    });

    let win_pace = main_window.as_weak();
    let last_report_pace = last_report.clone();
    main_window.on_apply_pacing_requested(move || {
        let Some(window) = win_pace.upgrade() else {
            return;
        };
        if let Ok(guard) = last_report_pace.lock() {
            if let Some(ref report) = *guard {
                let _ = clak_diagnostics::apply_pacing_profile(&report.pacing);
                let cfg = clak_engine::config::ClakConfig::load();
                window.set_pacing_preset_index(3);
                window.set_draftjs_delay_ms(cfg.pacing.draftjs_delay_ms as i32);
                window.set_draftjs_gap_ms(cfg.pacing.draftjs_gap_ms as i32);
                window.set_terminal_delay_ms(cfg.pacing.terminal_delay_ms as i32);
                window.set_terminal_gap_ms(cfg.pacing.terminal_gap_ms as i32);
                window.set_heavy_app_delay_ms(cfg.pacing.heavy_app_delay_ms as i32);
                window.set_heavy_app_gap_ms(cfg.pacing.heavy_app_gap_ms as i32);
                window.set_pacing_needs_tune(false);
                window.set_saved_toast(true);
                let win_toast = win_pace.clone();
                slint::Timer::single_shot(std::time::Duration::from_millis(1500), move || {
                    if let Some(w) = win_toast.upgrade() {
                        w.set_saved_toast(false);
                    }
                });
                let win_async = win_pace.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win_async.upgrade() {
                            if w.get_doctor_has_scanned() {
                                w.invoke_doctor_scan_requested();
                            }
                        }
                    });
                });
            }
        }
    });

    let win_startup = main_window.as_weak();
    let last_report_startup = last_report.clone();
    std::thread::spawn(move || {
        let report = clak_diagnostics::run_diagnostics();
        let is_healthy = report.is_healthy();
        let time_tag = chrono::Local::now().format("%H:%M:%S").to_string();
        let now = chrono::Local::now()
            .format("%H:%M:%S · %d/%m/%Y")
            .to_string();
        let lines = format_doctor_lines(&report, &time_tag);

        let mut score: i32 = 100;
        score -= (report.issues.len() as i32) * 25;
        score -= (report.warnings.len() as i32) * 10;
        let score = score.clamp(0, 100);

        let (score_level, score_label, score_color) = if score >= 90 {
            (
                0,
                "Khỏe mạnh",
                slint::Color::from_argb_u8(255, 16, 185, 129),
            )
        } else if score >= 60 {
            (
                1,
                "Cần lưu ý",
                slint::Color::from_argb_u8(255, 245, 158, 11),
            )
        } else {
            (2, "Nguy hiểm", slint::Color::from_argb_u8(255, 239, 68, 68))
        };

        let summary = if report.issues.is_empty() && report.warnings.is_empty() {
            "Tất cả kiểm tra đều hoàn hảo. Clak sẵn sàng hoạt động tối ưu.".to_string()
        } else {
            format!(
                "Phát hiện {} vấn đề cần xử lý, {} khuyến nghị lưu ý.",
                report.issues.len(),
                report.warnings.len()
            )
        };

        let is_deleted = report.memory.is_deleted_inode;
        let addon_not_loaded = report.daemon.fcitx5_running && !report.memory.clak_loaded;
        let needs_setup = !report.profile.clak_in_profile || !report.profile.is_default_im;
        let autostart_needs = !report.autostart.is_enabled;
        let permissions_needs =
            !report.permissions.uinput_writable || report.permissions.input_readable_count == 0;
        let wps_fix = report.wps.installed && !report.wps.configured;
        let jb_fix = report.jetbrains.installed && !report.jetbrains.unconfigured_ides.is_empty();

        let current_im_name = report
            .daemon
            .current_im
            .clone()
            .unwrap_or_else(|| "Không rõ".to_string());
        let im_switch =
            report.daemon.fcitx5_running && report.daemon.current_im.as_deref() != Some("clak");

        let cur_cfg = clak_engine::config::ClakConfig::load();
        let pacing_differs = cur_cfg.pacing.preset != "custom"
            || cur_cfg.pacing.draftjs_delay_ms != report.pacing.draftjs_post_delay_ms
            || cur_cfg.pacing.terminal_delay_ms != report.pacing.terminal_post_delay_ms
            || cur_cfg.pacing.draftjs_gap_ms != report.pacing.gap_ms;

        let pacing_tune_desc = format!(
            "Màn hình {:.0}Hz (budget {:.1}ms). Đề xuất: Terminal {}ms, Browser {}ms, DraftJS {}ms (Gap {}ms)",
            report.display.effective_refresh_rate,
            report.display.frame_budget_ms,
            report.pacing.terminal_post_delay_ms,
            report.pacing.browser_post_delay_ms,
            report.pacing.draftjs_post_delay_ms,
            report.pacing.gap_ms
        );

        let env_conflict = report.environment.gtk_im_module.as_deref() == Some("ibus")
            || report.environment.qt_im_module.as_deref() == Some("ibus")
            || report
                .environment
                .xmodifiers
                .as_deref()
                .is_some_and(|x| x.contains("ibus"))
            || !report.environment.env_file_exists;

        let has_fixable = !report.is_healthy()
            || is_deleted
            || addon_not_loaded
            || needs_setup
            || autostart_needs
            || permissions_needs
            || wps_fix
            || jb_fix
            || im_switch
            || pacing_differs
            || env_conflict;

        let report_for_ui = report.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = win_startup.upgrade() {
                let lines_model = Rc::new(VecModel::from(lines));
                w.set_doctor_lines(lines_model.into());
                w.set_doctor_last_scanned(now.into());
                w.set_doctor_summary_text(summary.into());
                w.set_doctor_is_healthy(is_healthy);
                w.set_doctor_score_level(score_level);
                w.set_doctor_score_text(score.to_string().into());
                w.set_doctor_score_label(score_label.into());
                w.set_doctor_score_color(score_color);
                w.set_doctor_has_scanned(true);
                w.set_doctor_is_scanning(false);

                if is_deleted {
                    w.set_reload_needed(true);
                }
                w.set_addon_needs_load(addon_not_loaded && !is_deleted);
                w.set_profile_needs_setup(needs_setup);
                w.set_env_needs_fix(env_conflict);
                w.set_autostart_needs_fix(autostart_needs);
                w.set_permissions_needs_fix(permissions_needs);
                w.set_wps_needs_fix(wps_fix);
                w.set_jetbrains_needs_fix(jb_fix);
                w.set_im_switch_needed(im_switch);
                w.set_current_im_name(current_im_name.into());
                w.set_pacing_needs_tune(pacing_differs);
                w.set_pacing_tune_description(pacing_tune_desc.into());
                w.set_has_fixable_issues(has_fixable);

                // nếu chưa thêm vào profile hoặc chưa nạp addon, tự động mở tab Chẩn đoán để hướng dẫn người dùng
                if needs_setup || (addon_not_loaded && !is_deleted) {
                    w.set_selected_tab(4);
                }

                if let Ok(mut guard) = last_report_startup.lock() {
                    *guard = Some(report_for_ui);
                }
            }
        });
    });

    // run diagnostics immediately if preview flag is passed
    if std::env::args().any(|a| a == "--preview-doctor") {
        let report = clak_diagnostics::run_diagnostics();
        let is_healthy = report.is_healthy();
        let time_tag = chrono::Local::now().format("%H:%M:%S").to_string();
        let now = chrono::Local::now()
            .format("%H:%M:%S · %d/%m/%Y")
            .to_string();
        let lines = format_doctor_lines(&report, &time_tag);

        let mut score: i32 = 100;
        score -= (report.issues.len() as i32) * 25;
        score -= (report.warnings.len() as i32) * 10;
        let score = score.clamp(0, 100);

        let (score_level, score_label, score_color) = if score >= 90 {
            (
                0,
                "Khỏe mạnh",
                slint::Color::from_argb_u8(255, 16, 185, 129),
            )
        } else if score >= 60 {
            (
                1,
                "Cần lưu ý",
                slint::Color::from_argb_u8(255, 245, 158, 11),
            )
        } else {
            (2, "Nguy hiểm", slint::Color::from_argb_u8(255, 239, 68, 68))
        };

        let summary = if report.issues.is_empty() && report.warnings.is_empty() {
            "Tất cả kiểm tra đều hoàn hảo. Clak sẵn sàng hoạt động tối ưu.".to_string()
        } else {
            format!(
                "Phát hiện {} vấn đề cần xử lý, {} khuyến nghị lưu ý.",
                report.issues.len(),
                report.warnings.len()
            )
        };

        let lines_rc = std::rc::Rc::new(slint::VecModel::from(lines));
        main_window.set_doctor_lines(lines_rc.into());
        main_window.set_doctor_last_scanned(now.into());
        main_window.set_doctor_summary_text(summary.into());
        main_window.set_doctor_is_healthy(is_healthy);
        main_window.set_doctor_score_level(score_level);
        main_window.set_doctor_score_text(score.to_string().into());
        main_window.set_doctor_score_label(score_label.into());
        main_window.set_doctor_score_color(score_color);
        main_window.set_doctor_has_scanned(true);
        main_window.set_doctor_is_scanning(false);
        main_window.set_reload_needed(report.memory.is_deleted_inode);
        let addon_not_loaded = report.daemon.fcitx5_running && !report.memory.clak_loaded;
        main_window.set_addon_needs_load(addon_not_loaded && !report.memory.is_deleted_inode);
        main_window.set_profile_needs_setup(
            !report.profile.clak_in_profile || !report.profile.is_default_im,
        );
        main_window.set_wps_needs_fix(report.wps.installed && !report.wps.configured);
        main_window.set_jetbrains_needs_fix(
            report.jetbrains.installed && !report.jetbrains.unconfigured_ides.is_empty(),
        );
        main_window.set_selected_tab(4);
        if let Ok(mut guard) = last_report.lock() {
            *guard = Some(report);
        }
    }

    // auto-detect window resize to scale text and UI for full window (Hyprland / tiling / maximized)
    let resize_window_weak = main_window.as_weak();
    let resize_timer = slint::Timer::default();
    resize_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(150),
        move || {
            if let Some(w) = resize_window_weak.upgrade() {
                let width = w.window().size().width;
                let scale = w.window().scale_factor();
                let logical_width = if scale > 0.0 {
                    width as f32 / scale
                } else {
                    width as f32
                };
                let is_wide = logical_width >= 1100.0;
                if w.get_is_wide() != is_wide {
                    w.set_is_wide(is_wide);
                }
            }
        },
    );

    main_window.run()?;
    Ok(())
}
