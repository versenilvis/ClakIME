use clak_engine::config::{ClakConfig, MacroItem};
use clak_engine::{ClakContext, Method, ACTION_FORWARD, ACTION_REPLACE};

fn type_text(ctx: &mut ClakContext, text: &str) -> Vec<(i32, usize, String)> {
    let mut results = Vec::new();
    for ch in text.chars() {
        let s = ch.to_string();
        let sym = ch as u32;
        let action = ctx.process_key(sym, &s, false, None, 0, 0);
        let commit = if !action.commit_str.is_null() {
            unsafe { std::ffi::CStr::from_ptr(action.commit_str) }
                .to_str()
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };
        results.push((action.action_type, action.delete_count, commit));
    }
    results
}

#[test]
fn test_toggle_modern_tone_telex() {
    let mut cfg_modern = ClakConfig::default();
    cfg_modern.spelling.modern_tone = true;

    let mut cfg_trad = ClakConfig::default();
    cfg_trad.spelling.modern_tone = false;

    // test modern tone: 'hoas' -> 'hoá' (common prefix 'ho', replaces 'a' with 'á')
    let mut ctx_modern = ClakContext::new(Method::Telex);
    ctx_modern.apply_config(&cfg_modern);
    type_text(&mut ctx_modern, "hoa");
    let act_modern = type_text(&mut ctx_modern, "s");
    assert_eq!(act_modern[0].0, ACTION_REPLACE);
    assert_eq!(act_modern[0].2, "á");

    // test traditional tone: 'hoas' -> 'hóa'
    let mut ctx_trad = ClakContext::new(Method::Telex);
    ctx_trad.apply_config(&cfg_trad);
    type_text(&mut ctx_trad, "hoa");
    let act_trad = type_text(&mut ctx_trad, "s");
    assert_eq!(act_trad[0].0, ACTION_REPLACE);
    assert_eq!(act_trad[0].2, "óa");
}

#[test]
fn test_toggle_macro_expansion() {
    let mut cfg_on = ClakConfig::default();
    cfg_on.macros.enabled = true;
    cfg_on.macros.items.push(MacroItem {
        trigger: "btw".to_string(),
        replace: "by the way".to_string(),
    });

    let mut cfg_off = cfg_on.clone();
    cfg_off.macros.enabled = false;

    // macros enabled: typing 'btw ' should replace 'btw' with 'by the way '
    let mut ctx_on = ClakContext::new(Method::Telex);
    ctx_on.apply_config(&cfg_on);
    type_text(&mut ctx_on, "btw");
    let acts_on = type_text(&mut ctx_on, " ");
    assert_eq!(acts_on[0].0, ACTION_REPLACE);
    assert_eq!(acts_on[0].1, 3);
    assert_eq!(acts_on[0].2, "by the way ");

    // macros disabled: typing 'btw ' forwards space
    let mut ctx_off = ClakContext::new(Method::Telex);
    ctx_off.apply_config(&cfg_off);
    type_text(&mut ctx_off, "btw");
    let acts_off = type_text(&mut ctx_off, " ");
    assert_eq!(acts_off[0].0, ACTION_FORWARD);
}

#[test]
fn test_toggle_double_space_period() {
    let mut cfg_on = ClakConfig::default();
    cfg_on.typing.double_space_period = true;

    let mut cfg_off = ClakConfig::default();
    cfg_off.typing.double_space_period = false;

    // double space on: space after a space in surrounding text replaces with period
    let mut ctx_on = ClakContext::new(Method::Telex);
    ctx_on.apply_config(&cfg_on);
    let act_on = ctx_on.process_key(0x20, " ", false, Some("word "), 5, 5);
    assert_eq!(act_on.action_type, ACTION_REPLACE);
    assert_eq!(act_on.delete_count, 1);
    let commit = unsafe { std::ffi::CStr::from_ptr(act_on.commit_str) }
        .to_str()
        .unwrap_or("");
    assert_eq!(commit, ". ");

    // double space off: space simply forwards
    let mut ctx_off = ClakContext::new(Method::Telex);
    ctx_off.apply_config(&cfg_off);
    let act_off = ctx_off.process_key(0x20, " ", false, Some("word "), 5, 5);
    assert_eq!(act_off.action_type, ACTION_FORWARD);
}

#[test]
fn test_toggle_auto_restore() {
    let mut cfg_on = ClakConfig::default();
    cfg_on.spelling.auto_restore = true;

    let mut cfg_off = ClakConfig::default();
    cfg_off.spelling.auto_restore = false;

    // auto restore on: non-vietnamese english word restores raw keys
    let mut ctx_on = ClakContext::new(Method::Telex);
    ctx_on.apply_config(&cfg_on);
    let acts_on = type_text(&mut ctx_on, "wording");
    let last_on = acts_on.last().unwrap();
    // wording restores raw English spelling
    assert!(!last_on.2.contains("ơ"));

    // auto restore off: telex marks remain
    let mut ctx_off = ClakContext::new(Method::Telex);
    cfg_off.general.short_w = true;
    ctx_off.apply_config(&cfg_off);
    type_text(&mut ctx_off, "w");
    let acts_off = type_text(&mut ctx_off, "o");
    assert_eq!(acts_off[0].2, "ơ");
}

#[test]
fn test_toggle_auto_capitalize() {
    let mut cfg_on = ClakConfig::default();
    cfg_on.typing.auto_capitalize = true;

    let mut cfg_off = ClakConfig::default();
    cfg_off.typing.auto_capitalize = false;

    // auto capitalize on: capitalize after period and space
    let mut ctx_on = ClakContext::new(Method::Telex);
    ctx_on.apply_config(&cfg_on);
    type_text(&mut ctx_on, ".");
    type_text(&mut ctx_on, " ");
    let acts_a = type_text(&mut ctx_on, "a");
    assert_eq!(acts_a[0].0, ACTION_REPLACE);
    assert_eq!(acts_a[0].2, "A");

    // question mark boundary
    type_text(&mut ctx_on, "?");
    type_text(&mut ctx_on, " ");
    let acts_b = type_text(&mut ctx_on, "b");
    assert_eq!(acts_b[0].0, ACTION_REPLACE);
    assert_eq!(acts_b[0].2, "B");

    // no space after period does not capitalize
    type_text(&mut ctx_on, ".");
    let acts_c = type_text(&mut ctx_on, "c");
    assert_eq!(acts_c[0].0, ACTION_FORWARD);

    // backspace cancels capitalization
    type_text(&mut ctx_on, ".");
    type_text(&mut ctx_on, " ");
    ctx_on.process_key(0xff08, "", false, None, 0, 0);
    let acts_d = type_text(&mut ctx_on, "d");
    assert_eq!(acts_d[0].0, ACTION_FORWARD);

    // enter cancels capitalization
    type_text(&mut ctx_on, ".");
    type_text(&mut ctx_on, " ");
    ctx_on.process_key(0xff0d, "\n", false, None, 0, 0);
    let acts_e = type_text(&mut ctx_on, "e");
    assert_eq!(acts_e[0].0, ACTION_FORWARD);

    // auto capitalize off: always forwards lowercase
    let mut ctx_off = ClakContext::new(Method::Telex);
    ctx_off.apply_config(&cfg_off);
    type_text(&mut ctx_off, ".");
    type_text(&mut ctx_off, " ");
    let acts_off = type_text(&mut ctx_off, "a");
    assert_eq!(acts_off[0].0, ACTION_FORWARD);
}

#[test]
fn test_debug_log_toggle_and_secure_permissions() {
    let mut cfg_on = ClakConfig::default();
    cfg_on.advanced.debug_log = true;

    let mut ctx_on = ClakContext::new(Method::Telex);
    ctx_on.apply_config(&cfg_on);

    // type a key with debug log on and verify permissions
    let _ = type_text(&mut ctx_on, "a");

    let path = std::path::Path::new("/tmp/clak.log");
    if path.exists() {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(path).unwrap();
        let permissions = meta.mode() & 0o777;
        assert_eq!(permissions, 0o600);
    }
}
