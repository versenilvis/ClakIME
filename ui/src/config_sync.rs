use crate::shortcuts::shortcut_display_to_id;
use crate::{AppRuleData, MacroItemData, MainWindow};
use clak_engine::config::{ClakConfig, MacroItem};
use slint::{Model, VecModel};
use std::rc::Rc;

pub fn save_config(
    window: &MainWindow,
    app_rules_model: &Rc<VecModel<AppRuleData>>,
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
    let toggle_sc_display = window.get_toggle_shortcut_text().to_string();
    let toggle_sc_str = shortcut_display_to_id(&toggle_sc_display);
    let switch_sc_str = match window.get_switch_shortcut_index() {
        1 => "none",
        _ => "ctrl_space",
    };

    let mut saved_rules = std::collections::HashMap::new();
    for i in 0..app_rules_model.row_count() {
        if let Some(rule) = app_rules_model.row_data(i) {
            let n = rule.name.trim().to_string();
            let m = rule.mode.trim().to_string();
            if !n.is_empty() {
                saved_rules.insert(n, m);
            }
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

    cfg_to_save.spelling.enabled = window.get_auto_restore();
    cfg_to_save.spelling.auto_restore = window.get_auto_restore();
    cfg_to_save.spelling.auto_restore_english = window.get_auto_restore_english();
    cfg_to_save.spelling.allow_foreign_consonants = window.get_allow_foreign_consonants();
    cfg_to_save.spelling.modern_tone = window.get_modern_tone();
    cfg_to_save.spelling.autocorrect = window.get_autocorrect();

    cfg_to_save.typing.double_space_period = window.get_double_space_period();
    cfg_to_save.typing.auto_capitalize = window.get_auto_capitalize();
    cfg_to_save.typing.shift_autofix = window.get_shift_autofix();
    cfg_to_save.macros.enabled = window.get_macros_enabled();
    cfg_to_save.macros.items = saved_macros;

    cfg_to_save.shortcuts.toggle_vietnamese = toggle_sc_str.to_string();
    cfg_to_save.shortcuts.switch_method = switch_sc_str.to_string();

    cfg_to_save.per_app.remember_state = window.get_remember_app_state();
    cfg_to_save.per_app.rules = saved_rules;

    cfg_to_save.advanced.autostart = window.get_autostart_system();
    cfg_to_save.advanced.uinput_ack = window.get_uinput_ack();
    cfg_to_save.advanced.debug_log = window.get_debug_log();

    let pacing_str = match window.get_pacing_preset_index() {
        0 => "fast",
        1 => "balanced",
        2 => "safe",
        3 => "custom",
        _ => "balanced",
    };
    cfg_to_save.pacing.adaptive = window.get_adaptive_pacing();
    cfg_to_save.pacing.preset = pacing_str.to_string();
    cfg_to_save.pacing.draftjs_delay_ms = window.get_draftjs_delay_ms() as u32;
    cfg_to_save.pacing.draftjs_gap_ms = window.get_draftjs_gap_ms() as u32;
    cfg_to_save.pacing.terminal_delay_ms = window.get_terminal_delay_ms() as u32;
    cfg_to_save.pacing.terminal_gap_ms = window.get_terminal_gap_ms() as u32;
    cfg_to_save.pacing.heavy_app_delay_ms = window.get_heavy_app_delay_ms() as u32;
    cfg_to_save.pacing.heavy_app_gap_ms = window.get_heavy_app_gap_ms() as u32;

    cfg_to_save.update.auto_update = window.get_auto_update_enabled();
    cfg_to_save.update.poll_index = window.get_update_poll_index();

    let _ = cfg_to_save.save();
}
