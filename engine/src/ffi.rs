use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use crate::core::{dict, ClakCore, Method};
use crate::config;
use crate::session;
use crate::autofix;
use crate::charset;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_new(method: i32) -> *mut ClakCore {
    let m = match method {
        0 => Method::Telex,
        1 => Method::Vni,
        2 => Method::Viqr,
        3 => Method::TeipVni,
        _ => Method::Telex,
    };
    Box::into_raw(Box::new(ClakCore::new(m)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_free(engine: *mut ClakCore) {
    if !engine.is_null() {
        unsafe {
            drop(Box::from_raw(engine));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_method(engine: *mut ClakCore, method: i32) {
    if engine.is_null() {
        return;
    }
    let m = match method {
        0 => Method::Telex,
        1 => Method::Vni,
        2 => Method::Viqr,
        3 => Method::TeipVni,
        _ => Method::Telex,
    };
    unsafe {
        (*engine).set_method(m);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_tone_style(e: *mut ClakCore, v: i32) {
    if !e.is_null() {
        unsafe {
            (*e).set_modern(v != 0);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_free_marking(e: *mut ClakCore, v: i32) {
    if !e.is_null() {
        unsafe {
            (*e).set_modern(v != 0);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_short_w(e: *mut ClakCore, v: i32) {
    if !e.is_null() {
        unsafe {
            (*e).set_short_w(v != 0);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_auto_restore(e: *mut ClakCore, v: i32) {
    if !e.is_null() {
        unsafe {
            (*e).set_auto_restore(v != 0);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_auto_restore_english(e: *mut ClakCore, v: i32) {
    if !e.is_null() {
        unsafe {
            (*e).set_auto_restore_english(v != 0);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_allow_foreign_consonants(e: *mut ClakCore, v: i32) {
    if !e.is_null() {
        unsafe {
            (*e).set_allow_foreign_consonants(v != 0);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_dict(e: *mut ClakCore, v: i32) {
    if !e.is_null() {
        unsafe {
            (*e).set_dict(v != 0);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_add_word(e: *mut ClakCore, word: *const c_char) {
    if e.is_null() || word.is_null() {
        return;
    }
    let s = match unsafe { CStr::from_ptr(word) }.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    unsafe {
        (*e).add_word(s);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_clear_words(e: *mut ClakCore) {
    if !e.is_null() {
        unsafe {
            (*e).clear_words();
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_dict_words() -> *mut c_char {
    CString::new(dict().join("\n"))
        .unwrap_or_default()
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_set_bracket_uo(_e: *mut ClakCore, _v: i32) {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_transform(
    engine: *const ClakCore,
    input: *const c_char,
) -> *mut c_char {
    if engine.is_null() || input.is_null() {
        return std::ptr::null_mut();
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let e = unsafe { &*engine };
        let s = match unsafe { CStr::from_ptr(input) }.to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };
        CString::new(e.transform(s)).unwrap_or_default().into_raw()
    }));
    result.unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_autofix_text(
    input: *const c_char,
    method: i32,
    modern: i32,
    changed_out: *mut i32,
) -> *mut c_char {
    if input.is_null() {
        if !changed_out.is_null() {
            unsafe { *changed_out = 0 };
        }
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(input) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            if !changed_out.is_null() {
                unsafe { *changed_out = 0 };
            }
            return std::ptr::null_mut();
        }
    };
    let m = match method {
        0 => Method::Telex,
        1 => Method::Vni,
        2 => Method::Viqr,
        3 => Method::TeipVni,
        _ => Method::Telex,
    };
    let (fixed, was_changed) = autofix::autofix_text(s, m, modern != 0);
    if !changed_out.is_null() {
        unsafe { *changed_out = if was_changed { 1 } else { 0 } };
    }
    match CString::new(fixed) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_charset_encode(
    input: *const c_char,
    charset: i32,
    out_len: *mut usize,
) -> *mut u8 {
    if input.is_null() || out_len.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(input) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            unsafe {
                *out_len = 0;
            }
            return std::ptr::null_mut();
        }
    };
    let cs = match charset {
        0 => charset::VietCharset::Unicode,
        1 => charset::VietCharset::TCVN3,
        2 => charset::VietCharset::VNIWin,
        3 => charset::VietCharset::WinCP1258,
        4 => charset::VietCharset::VIQR,
        _ => charset::VietCharset::Unicode,
    };
    let encoded = charset::encode(s, cs);
    let len = encoded.len();
    if len == 0 {
        unsafe {
            *out_len = 0;
        }
        return std::ptr::null_mut();
    }
    let ptr = libc::malloc(len).cast::<u8>();
    if ptr.is_null() {
        unsafe {
            *out_len = 0;
        }
        return std::ptr::null_mut();
    }
    unsafe {
        std::ptr::copy_nonoverlapping(encoded.as_ptr(), ptr, len);
        *out_len = len;
    }
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_charset_decode(
    input: *const u8,
    len: usize,
    charset: i32,
) -> *mut c_char {
    if input.is_null() {
        return std::ptr::null_mut();
    }
    let bytes = unsafe { std::slice::from_raw_parts(input, len) };
    let cs = match charset {
        0 => charset::VietCharset::Unicode,
        1 => charset::VietCharset::TCVN3,
        2 => charset::VietCharset::VNIWin,
        3 => charset::VietCharset::WinCP1258,
        4 => charset::VietCharset::VIQR,
        _ => charset::VietCharset::Unicode,
    };
    let decoded = charset::decode(bytes, cs);
    CString::new(decoded).unwrap_or_default().into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_charset_remove_tone(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(input) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    CString::new(charset::remove_tone(s))
        .unwrap_or_default()
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_charset_free_buf(ptr: *mut u8) {
    if !ptr.is_null() {
        unsafe {
            libc::free(ptr.cast());
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_core_is_valid(s: *const c_char) -> i32 {
    if s.is_null() {
        return 0;
    }
    let s = match unsafe { CStr::from_ptr(s) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    if ClakCore::is_valid(s) {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_path() -> *mut c_char {
    let p = config::config_path().to_string_lossy().to_string();
    CString::new(p).unwrap_or_default().into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_load() -> *mut config::ClakConfig {
    Box::into_raw(Box::new(config::ClakConfig::load()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_default() -> *mut config::ClakConfig {
    Box::into_raw(Box::new(config::ClakConfig::default()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_free(cfg: *mut config::ClakConfig) {
    if !cfg.is_null() {
        drop(Box::from_raw(cfg));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_app_mode(
    cfg: *const config::ClakConfig,
    app_name: *const c_char,
) -> i32 {
    if cfg.is_null() || app_name.is_null() {
        return 0;
    }
    let app = match CStr::from_ptr(app_name).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    (*cfg).per_app.get_mode(app)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_is_app_excluded(
    cfg: *const config::ClakConfig,
    app_name: *const c_char,
) -> bool {
    clak_config_get_app_mode(cfg, app_name) == 3
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_remember_state(cfg: *const config::ClakConfig) -> bool {
    if cfg.is_null() {
        return true;
    }
    (*cfg).per_app.remember_state
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_uinput_ack(cfg: *const config::ClakConfig) -> bool {
    if cfg.is_null() {
        return false;
    }
    (*cfg).advanced.uinput_ack
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_debug_log(cfg: *const config::ClakConfig) -> bool {
    if cfg.is_null() {
        return false;
    }
    (*cfg).advanced.debug_log
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_adaptive_pacing(cfg: *const config::ClakConfig) -> bool {
    if cfg.is_null() {
        return true;
    }
    (*cfg).pacing.adaptive
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_pacing_preset(cfg: *const config::ClakConfig) -> i32 {
    if cfg.is_null() {
        return 3;
    }
    match (*cfg).pacing.preset.as_str() {
        "fast" => 0,
        "balanced" => 1,
        "safe" => 2,
        "custom" => 3,
        _ => 3,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_draftjs_delay_ms(cfg: *const config::ClakConfig) -> u32 {
    if cfg.is_null() {
        return 16;
    }
    match (*cfg).pacing.preset.as_str() {
        "fast" => 10,
        "safe" => 24,
        "balanced" => 16,
        "custom" => (*cfg).pacing.draftjs_delay_ms,
        _ => (*cfg).pacing.draftjs_delay_ms,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_draftjs_gap_ms(cfg: *const config::ClakConfig) -> u32 {
    if cfg.is_null() {
        return 8;
    }
    match (*cfg).pacing.preset.as_str() {
        "fast" => 4,
        "safe" => 12,
        "balanced" => 8,
        "custom" => (*cfg).pacing.draftjs_gap_ms,
        _ => (*cfg).pacing.draftjs_gap_ms,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_terminal_delay_ms(cfg: *const config::ClakConfig) -> u32 {
    if cfg.is_null() {
        return 12;
    }
    match (*cfg).pacing.preset.as_str() {
        "fast" => 6,
        "safe" => 20,
        "balanced" => 12,
        "custom" => (*cfg).pacing.terminal_delay_ms,
        _ => (*cfg).pacing.terminal_delay_ms,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_terminal_gap_ms(cfg: *const config::ClakConfig) -> u32 {
    if cfg.is_null() {
        return 3;
    }
    match (*cfg).pacing.preset.as_str() {
        "fast" => 2,
        "safe" => 5,
        "balanced" => 3,
        "custom" => (*cfg).pacing.terminal_gap_ms,
        _ => (*cfg).pacing.terminal_gap_ms,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_heavy_app_delay_ms(cfg: *const config::ClakConfig) -> u32 {
    if cfg.is_null() {
        return 15;
    }
    match (*cfg).pacing.preset.as_str() {
        "fast" => 10,
        "safe" => 22,
        "balanced" => 15,
        "custom" => (*cfg).pacing.heavy_app_delay_ms,
        _ => (*cfg).pacing.heavy_app_delay_ms,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_heavy_app_gap_ms(cfg: *const config::ClakConfig) -> u32 {
    if cfg.is_null() {
        return 4;
    }
    match (*cfg).pacing.preset.as_str() {
        "fast" => 3,
        "safe" => 6,
        "balanced" => 4,
        "custom" => (*cfg).pacing.heavy_app_gap_ms,
        _ => (*cfg).pacing.heavy_app_gap_ms,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_max_surr_lag_ms(cfg: *const config::ClakConfig) -> u64 {
    if cfg.is_null() {
        200
    } else {
        (*cfg).pacing.max_surr_lag_ms
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_max_surr_window_chars(
    cfg: *const config::ClakConfig,
) -> usize {
    if cfg.is_null() {
        8
    } else {
        (*cfg).pacing.max_surr_window_chars
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_shift_autofix(cfg: *const config::ClakConfig) -> bool {
    if cfg.is_null() {
        return false;
    }
    (*cfg).typing.shift_autofix
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_set_shift_autofix(cfg: *mut config::ClakConfig, v: bool) {
    if !cfg.is_null() {
        unsafe { (*cfg).typing.shift_autofix = v };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_autocorrect(cfg: *const config::ClakConfig) -> bool {
    if cfg.is_null() {
        return true;
    }
    (*cfg).spelling.autocorrect
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_set_autocorrect(cfg: *mut config::ClakConfig, v: bool) {
    if !cfg.is_null() {
        unsafe { (*cfg).spelling.autocorrect = v };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_set_auto_restore(cfg: *mut config::ClakConfig, v: bool) {
    if !cfg.is_null() {
        unsafe { (*cfg).spelling.auto_restore = v };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_auto_restore_english(
    cfg: *const config::ClakConfig,
) -> bool {
    if cfg.is_null() {
        return false;
    }
    (*cfg).spelling.auto_restore_english
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_set_auto_restore_english(
    cfg: *mut config::ClakConfig,
    v: bool,
) {
    if !cfg.is_null() {
        unsafe { (*cfg).spelling.auto_restore_english = v };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_allow_foreign_consonants(
    cfg: *const config::ClakConfig,
) -> bool {
    if cfg.is_null() {
        return false;
    }
    (*cfg).spelling.allow_foreign_consonants
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_set_allow_foreign_consonants(
    cfg: *mut config::ClakConfig,
    v: bool,
) {
    if !cfg.is_null() {
        unsafe { (*cfg).spelling.allow_foreign_consonants = v };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_set_spelling(cfg: *mut config::ClakConfig, v: bool) {
    if !cfg.is_null() {
        unsafe { (*cfg).spelling.enabled = v };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_startup_mode(cfg: *const config::ClakConfig) -> i32 {
    if cfg.is_null() {
        return 0;
    }
    if (*cfg).general.startup_mode == "english" {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_method(cfg: *const config::ClakConfig) -> i32 {
    if cfg.is_null() {
        return 0;
    }
    match (*cfg).general.method.as_str() {
        "vni" => 1,
        "viqr" => 2,
        "teip_vni" => 3,
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_toggle_shortcut(
    cfg: *const config::ClakConfig,
) -> *mut c_char {
    let sc = if cfg.is_null() {
        "ctrl_shift"
    } else {
        (*cfg).shortcuts.toggle_vietnamese.as_str()
    };
    CString::new(sc).unwrap_or_default().into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_config_get_switch_shortcut(
    cfg: *const config::ClakConfig,
) -> *mut c_char {
    let sc = if cfg.is_null() {
        "ctrl_space"
    } else {
        (*cfg).shortcuts.switch_method.as_str()
    };
    CString::new(sc).unwrap_or_default().into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_new(method: i32) -> *mut session::ClakSession {
    let m = match method {
        1 => Method::Vni,
        2 => Method::Viqr,
        3 => Method::TeipVni,
        _ => Method::Telex,
    };
    Box::into_raw(Box::new(session::ClakSession::new(m)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_free(session: *mut session::ClakSession) {
    if !session.is_null() {
        unsafe {
            drop(Box::from_raw(session));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_reset(session: *mut session::ClakSession, force: bool) {
    if let Some(s) = unsafe { session.as_mut() } {
        s.reset(force);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_apply_config(
    session: *mut session::ClakSession,
    cfg: *const config::ClakConfig,
) {
    if let (Some(s), Some(c)) = (unsafe { session.as_mut() }, unsafe { cfg.as_ref() }) {
        s.apply_config(c);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_is_deleting(session: *const session::ClakSession) -> bool {
    if let Some(s) = unsafe { session.as_ref() } {
        s.is_deleting()
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_buffered_keys_count(
    session: *const session::ClakSession,
) -> usize {
    if let Some(s) = unsafe { session.as_ref() } {
        s.buffered_keys_count()
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_expected_backspaces(
    session: *const session::ClakSession,
) -> usize {
    if let Some(s) = unsafe { session.as_ref() } {
        s.expected_backspaces()
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_current_backspace_count(
    session: *const session::ClakSession,
) -> usize {
    if let Some(s) = unsafe { session.as_ref() } {
        s.current_backspace_count()
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_pending_commit(
    session: *const session::ClakSession,
) -> *const c_char {
    if let Some(s) = unsafe { session.as_ref() } {
        s.pending_commit_c_str()
    } else {
        c"".as_ptr()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_process_key(
    session: *mut session::ClakSession,
    sym: u32,
    key_str: *const c_char,
    has_ctrl_alt: bool,
    surrounding_text: *const c_char,
    cursor: usize,
    anchor: usize,
    deletion_mode: i32,
    autofill_extra: u32,
    uinput_post_delay_ms: u32,
    uinput_pre_delay_ms: u32,
    uinput_gap_ms: u32,
    uinput_hold_us: u32,
    safety_timeout_us: u64,
    timestamp_us: u64,
) -> session::ClakSessionCommandList {
    if let Some(s) = unsafe { session.as_mut() } {
        let k_str = if key_str.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(key_str) }.to_str().unwrap_or("")
        };
        let s_text = if surrounding_text.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(surrounding_text) }.to_str().ok()
        };
        s.process_key(
            sym,
            k_str,
            has_ctrl_alt,
            s_text,
            cursor,
            anchor,
            deletion_mode,
            autofill_extra,
            uinput_post_delay_ms,
            uinput_pre_delay_ms,
            uinput_gap_ms,
            uinput_hold_us,
            safety_timeout_us,
            timestamp_us,
        )
    } else {
        session::ClakSessionCommandList {
            count: 0,
            commands: std::ptr::null(),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_switch_to_forward_fallback(
    session: *mut session::ClakSession,
    delay_us: u64,
) -> session::ClakSessionCommandList {
    if let Some(s) = unsafe { session.as_mut() } {
        s.switch_to_forward_fallback(delay_us)
    } else {
        session::ClakSessionCommandList {
            count: 0,
            commands: std::ptr::null(),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_last_autofix_delete(
    session: *const session::ClakSession,
) -> *const std::os::raw::c_char {
    if let Some(s) = unsafe { session.as_ref() } {
        s.last_autofix_delete()
    } else {
        std::ptr::null()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_last_autofix_commit(
    session: *const session::ClakSession,
) -> *const std::os::raw::c_char {
    if let Some(s) = unsafe { session.as_ref() } {
        s.last_autofix_commit()
    } else {
        std::ptr::null()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_on_backspace_received(
    session: *mut session::ClakSession,
) -> bool {
    if let Some(s) = unsafe { session.as_mut() } {
        s.on_backspace_received()
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_on_sentinel_received(
    session: *mut session::ClakSession,
    timestamp_us: u64,
) -> session::ClakSessionCommandList {
    if let Some(s) = unsafe { session.as_mut() } {
        s.on_sentinel_received(timestamp_us)
    } else {
        session::ClakSessionCommandList {
            count: 0,
            commands: std::ptr::null(),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_session_on_timer_fired(
    session: *mut session::ClakSession,
    timer_id: u32,
    timestamp_us: u64,
) -> session::ClakSessionCommandList {
    if let Some(s) = unsafe { session.as_mut() } {
        s.on_timer_fired(timer_id, timestamp_us)
    } else {
        session::ClakSessionCommandList {
            count: 0,
            commands: std::ptr::null(),
        }
    }
}

