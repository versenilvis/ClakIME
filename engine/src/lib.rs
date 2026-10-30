pub mod autocorrect;
pub mod autofix;
pub mod charset;
pub mod config;
pub mod engine;
pub mod english_dict;
pub mod ime;
pub mod learned_typos;
pub mod spelling;
pub mod tables;
pub mod vseq;

pub use config::*;
pub use ime::*;

use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Telex,
    Vni,
    TeipVni,
    Viqr,
}

static DICT: OnceLock<Vec<&'static str>> = OnceLock::new();
static BASES: OnceLock<Vec<String>> = OnceLock::new();

pub(crate) fn dict() -> &'static Vec<&'static str> {
    DICT.get_or_init(|| include_str!("../data/vietnamese.cm.dict").lines().collect())
}

fn bases() -> &'static Vec<String> {
    BASES.get_or_init(|| {
        let mut v: Vec<String> = dict().iter().map(|w| charset::remove_tone(w)).collect();
        v.sort();
        v.dedup();
        v
    })
}

pub struct ClakCore {
    method: Method,
    modern: bool,
    short_w: bool,
    auto_restore: bool,
    dict: bool,
    user_words: HashSet<String>,
    user_bases: Vec<String>,
}

pub type ClakEngine = ClakCore;

impl ClakCore {
    pub fn new(method: Method) -> Self {
        ClakCore {
            method,
            modern: true,
            short_w: false,
            auto_restore: false,
            dict: false,
            user_words: HashSet::new(),
            user_bases: Vec::new(),
        }
    }

    pub fn set_method(&mut self, method: Method) {
        self.method = method;
    }
    pub fn method(&self) -> Method {
        self.method
    }
    pub fn set_modern(&mut self, v: bool) {
        self.modern = v;
    }
    pub fn modern(&self) -> bool {
        self.modern
    }
    pub fn set_short_w(&mut self, v: bool) {
        self.short_w = v;
    }
    pub fn short_w(&self) -> bool {
        self.short_w
    }
    pub fn set_auto_restore(&mut self, v: bool) {
        self.auto_restore = v;
    }
    pub fn auto_restore(&self) -> bool {
        self.auto_restore
    }
    pub fn set_dict(&mut self, v: bool) {
        self.dict = v;
    }

    pub fn add_word(&mut self, word: &str) {
        let w = word.to_lowercase();
        let base = charset::remove_tone(&w);
        self.user_words.insert(w);
        if let Err(pos) = self.user_bases.binary_search(&base) {
            self.user_bases.insert(pos, base);
        }
    }

    pub fn clear_words(&mut self) {
        self.user_words.clear();
        self.user_bases.clear();
    }

    fn dict_known(&self, word: &str) -> bool {
        let word = word.trim_start_matches(|c: char| c.is_ascii_digit());
        if word.is_empty() {
            return false;
        }
        let base = charset::remove_tone(word);
        if base != word {
            if self.user_words.contains(word) || dict().binary_search(&word).is_ok() {
                return true;
            }
            let user_match = match self.user_bases.binary_search(&base) {
                Ok(i) => i + 1 < self.user_bases.len() && self.user_bases[i + 1].starts_with(&base),
                Err(i) => i < self.user_bases.len() && self.user_bases[i].starts_with(&base),
            };
            if user_match {
                return true;
            }
            let bs = bases();
            match bs.binary_search(&base) {
                Ok(i) => i + 1 < bs.len() && bs[i + 1].starts_with(&base),
                Err(i) => i < bs.len() && bs[i].starts_with(&base),
            }
        } else {
            let user_match = match self.user_bases.binary_search(&base) {
                Ok(_) => true,
                Err(i) => i < self.user_bases.len() && self.user_bases[i].starts_with(&base),
            };
            if user_match {
                return true;
            }
            let bs = bases();
            match bs.binary_search(&base) {
                Ok(_) => true,
                Err(i) => i < bs.len() && bs[i].starts_with(&base),
            }
        }
    }

    pub fn transform(&self, input: &str) -> String {
        let composed = match self.method {
            Method::Telex => engine::convert_telex(input, self.modern, self.short_w),
            Method::Vni => engine::convert_vni(input, self.modern, self.short_w),
            Method::TeipVni => engine::convert_teip_vni(input, self.modern, self.short_w),
            Method::Viqr => engine::convert_viqr(input),
        };
        let all_ascii = composed.is_ascii();
        let known = if self.dict {
            self.dict_known(&composed.to_lowercase())
        } else {
            Self::is_valid(&composed)
        };
        let is_en = crate::english_dict::is_english(input);
        let swallowed_char =
            (!has_consecutive_modifiers(input) || is_en) && input.len() > composed.len();
        if self.auto_restore
            && (!all_ascii || swallowed_char || is_en)
            && composed != input
            && !known
            && !has_vn_markers(input)
        {
            input.to_string()
        } else {
            composed
        }
    }

    pub fn is_valid(s: &str) -> bool {
        spelling::is_valid_cvc(s)
    }
}

pub(crate) fn has_consecutive_modifiers(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    lower.contains("ss")
        || lower.contains("ff")
        || lower.contains("rr")
        || lower.contains("xx")
        || lower.contains("jj")
        || lower.contains("ww")
        || lower.contains("aa")
        || lower.contains("ee")
        || lower.contains("oo")
        || lower.contains("dd")
}

pub(crate) fn has_vn_markers(input: &str) -> bool {
    let lower = input.to_lowercase();
    let dd_marker = lower.starts_with("dd")
        || (lower.starts_with("dud") && lower.contains('o'))
        || lower.match_indices("dd").any(|(i, _)| {
            i > 0
                && !matches!(
                    lower[..i].chars().next_back(),
                    Some('a' | 'e' | 'i' | 'o' | 'u' | 'y')
                )
        });
    dd_marker
        || lower.contains("aaa")
        || lower.contains("eee")
        || lower.contains("ooo")
        || lower.contains("aww")
        || lower.contains("oww")
        || lower.contains("uww")
}

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
pub unsafe extern "C" fn clak_config_is_app_excluded(
    cfg: *const config::ClakConfig,
    app_name: *const c_char,
) -> bool {
    if cfg.is_null() || app_name.is_null() {
        return false;
    }
    let app = match CStr::from_ptr(app_name).to_str() {
        Ok(s) => s.to_lowercase(),
        Err(_) => return false,
    };
    let c = &*cfg;
    c.per_app
        .excluded_apps
        .iter()
        .any(|ex| app.contains(&ex.to_lowercase()))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn eng(auto_restore: bool, dict: bool) -> ClakCore {
        let mut e = ClakCore::new(Method::Telex);
        e.set_auto_restore(auto_restore);
        e.set_dict(dict);
        e
    }

    #[test]
    fn dict_restores_nonwords() {
        let e = eng(true, true);
        assert_eq!(e.transform("luwocs"), "luwocs");
        assert_eq!(e.transform("need"), "need");
        assert_eq!(e.transform("force"), "force");
    }

    #[test]
    fn dict_keeps_prefixes_realtime() {
        let e = eng(true, true);
        assert_eq!(e.transform("phuowc"), "phươc");
        assert_eq!(e.transform("phuowcs"), "phước");
        assert_eq!(e.transform("truwoc"), "trươc");
        assert_eq!(e.transform("luow"), "lươ");
        assert_eq!(e.transform("luowj"), "lượ");
        assert_eq!(e.transform("luowjc"), "lược");

        let mut e2 = eng(true, true);
        e2.add_word("lước");
        assert_eq!(e2.transform("luwoc"), "lươc");
    }

    #[test]
    fn dict_keeps_real_words() {
        let e = eng(true, true);
        assert_eq!(e.transform("truwocs"), "trước");
        assert_eq!(e.transform("khoong"), "không");
        assert_eq!(e.transform("Vieetj"), "Việt");
        assert_eq!(e.transform("luoojcw"), "lược");
        assert_eq!(e.transform("ginf"), "gìn");
        assert_eq!(e.transform("gifn"), "gìn");

        let e_rule = eng(true, false);
        assert_eq!(e_rule.transform("ginf"), "gìn");
        assert_eq!(e_rule.transform("gifn"), "gìn");
    }

    #[test]
    fn dict_keeps_abbreviations() {
        let e = eng(true, true);
        assert_eq!(e.transform("ddc"), "đc");
    }

    #[test]
    fn auto_restore_unikey_style_english_word() {
        let e = eng(true, false);
        assert_eq!(e.transform("dow"), "dơ");
        assert_eq!(e.transform("doww"), "dow");
        assert_eq!(e.transform("dowwnlo"), "downlo");
        assert_eq!(e.transform("dowwnload"), "download");
        assert_eq!(e.transform("dowwnloads"), "downloads");

        let e = eng(true, true);
        assert_eq!(e.transform("dowwnload"), "download");
    }

    #[test]
    fn test_doubled_modifier_english_words_and_un_toning() {
        // preserve doubled modifier letters for english words
        let e = eng(true, false);
        assert_eq!(e.transform("boss"), "boss");
        assert_eq!(e.transform("pass"), "pass");
        assert_eq!(e.transform("class"), "class");
        assert_eq!(e.transform("less"), "less");
        assert_eq!(e.transform("err"), "err");
        assert_eq!(e.transform("error"), "error");
        assert_eq!(e.transform("sorry"), "sorry");
        assert_eq!(e.transform("tomorrow"), "tomorrow");
        assert_eq!(e.transform("deepseek"), "deepseek");
        assert_eq!(e.transform("DeepSeek"), "DeepSeek");

        // un-toning telex cleanup
        assert_eq!(e.transform("mixx"), "mix");
        assert_eq!(e.transform("mixxer"), "mixer");
        assert_eq!(e.transform("fixx"), "fix");

        // contraction stems and tech identifiers
        assert_eq!(e.transform("doesn"), "doesn");
        assert_eq!(e.transform("doesnt"), "doesnt");
        assert_eq!(e.transform("didn"), "didn");
        assert_eq!(e.transform("didnt"), "didnt");
        assert_eq!(e.transform("wasn"), "wasn");
        assert_eq!(e.transform("wasnt"), "wasnt");
        assert_eq!(e.transform("isnt"), "isnt");
        assert_eq!(e.transform("nginx"), "nginx");
        assert_eq!(e.transform("docker"), "docker");

        let e = eng(true, true);
        assert_eq!(e.transform("boss"), "boss");
        assert_eq!(e.transform("error"), "error");
        assert_eq!(e.transform("mixx"), "mix");
    }

    #[test]
    fn test_foreign_initial_consonants_no_tone() {
        let e = eng(true, false);
        assert_eq!(e.transform("just"), "just");
        assert_eq!(e.transform("jus"), "jus");
        assert_eq!(e.transform("fast"), "fast");

        let e = eng(true, true);
        assert_eq!(e.transform("just"), "just");
        assert_eq!(e.transform("jus"), "jus");
        assert_eq!(e.transform("fast"), "fast");
    }

    #[test]
    fn test_invalid_initial_consonant_vowel_pairs_no_tone() {
        let e = eng(true, false);
        assert_eq!(e.transform("ghos"), "ghos");
        assert_eq!(e.transform("ghoss"), "ghoss");
        assert_eq!(e.transform("ghost"), "ghost");
        assert_eq!(e.transform("ghostty"), "ghostty");
        assert_eq!(e.transform("nghos"), "nghos");
        assert_eq!(e.transform("ghes"), "ghé");
        assert_eq!(e.transform("nghes"), "nghé");

        let e = eng(true, true);
        assert_eq!(e.transform("ghos"), "ghos");
        assert_eq!(e.transform("ghoss"), "ghoss");
        assert_eq!(e.transform("ghost"), "ghost");
        assert_eq!(e.transform("ghostty"), "ghostty");
    }

    #[test]
    fn auto_restore_shortw_ww_english_word() {
        let mut e = eng(true, false);
        e.set_short_w(true);
        assert_eq!(e.transform("w"), "ư");
        assert_eq!(e.transform("ww"), "w");
        assert_eq!(e.transform("wwa"), "wa");
        assert_eq!(e.transform("wway"), "way");
        assert_eq!(e.transform("wwayl"), "wayl");
        assert_eq!(e.transform("wwayla"), "wayla");
        assert_eq!(e.transform("wwayland"), "wayland");
        assert_eq!(e.transform("wwindow"), "window");

        // w after digits is a unit, not u-horn
        assert_eq!(e.transform("100w"), "100w");
        assert_eq!(e.transform("100W"), "100W");
        assert_eq!(e.transform("65w"), "65w");
        assert_eq!(e.transform("2w"), "2w");

        // digits starting a vietnamese word
        assert_eq!(e.transform("1casi"), "1cái");
        assert_eq!(e.transform("5trawm"), "5trăm");
        assert_eq!(e.transform("4tuoori"), "4tuổi");

        let mut e = eng(true, true);
        e.set_short_w(true);
        assert_eq!(e.transform("wwayland"), "wayland");
        assert_eq!(e.transform("100w"), "100w");
        assert_eq!(e.transform("1casi"), "1cái");
    }

    #[test]
    fn auto_restore_english_dd_words() {
        let e = eng(true, false);
        assert_eq!(e.transform("add"), "add");
        assert_eq!(e.transform("addr"), "addr");
        assert_eq!(e.transform("addre"), "addre");
        assert_eq!(e.transform("addres"), "addres");
        assert_eq!(e.transform("address"), "address");
        assert_eq!(e.transform("ladder"), "ladder");
        assert_eq!(e.transform("odd"), "odd");

        let e = eng(true, true);
        assert_eq!(e.transform("address"), "address");

        let e = eng(true, false);
        assert_eq!(e.transform("ddc"), "đc");
        assert_eq!(e.transform("ddi"), "đi");
        assert_eq!(e.transform("DDaf"), "Đà");
        assert_eq!(e.transform("vcdd"), "vcđ");
        assert_eq!(e.transform("kdd"), "kđ");
        assert_eq!(e.transform("cmdd"), "cmđ");
    }

    #[test]
    fn double_vowel_swap_survives_auto_restore() {
        let e = eng(true, false);
        assert_eq!(e.transform("lawsma"), "lấm");
        let e = eng(true, true);
        assert_eq!(e.transform("lawsma"), "lấm");
        assert_eq!(e.transform("luowjco"), "luộc");
    }

    #[test]
    fn dict_off_keeps_rule_based_behavior() {
        let e = eng(true, false);
        assert_eq!(e.transform("is"), "í");
        assert_eq!(e.transform("uaj"), "ụa");
        assert_eq!(e.transform("uja"), "ụa");
        assert_eq!(e.transform("luwocs"), "lước");
        assert_eq!(e.transform("need"), "need");
    }

    #[test]
    fn dict_user_words_override() {
        let mut e = eng(true, true);
        assert_eq!(e.transform("luwocs"), "luwocs");
        e.add_word("Lước");
        assert_eq!(e.transform("luwocs"), "lước");
        assert_eq!(e.transform("LUWOCS"), "LƯỚC");
    }

    #[test]
    fn dict_embedded_is_sorted() {
        let dict =
            DICT.get_or_init(|| include_str!("../data/vietnamese.cm.dict").lines().collect());
        assert!(dict.len() > 7000, "dict unexpectedly small: {}", dict.len());
        for w in dict.windows(2) {
            assert!(w[0] < w[1], "dict not sorted at {:?}", w);
        }
    }

    #[test]
    fn test_speedtest_and_different() {
        let mut cfg = crate::config::ClakConfig::default();
        cfg.spelling.auto_restore = true;
        cfg.spelling.modern_tone = false;

        let mut ctx = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx.apply_config(&cfg);
        let mut ctx_closes = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx_closes.apply_config(&cfg);
        let mut sim_closes = String::new();
        for ch in "closes".chars() {
            let s = ch.to_string();
            let act = ctx_closes.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_closes.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_closes.push_str(commit);
            } else if act.action_type == 0 {
                sim_closes.push(ch);
            }
        }
        assert_eq!(sim_closes, "closes");

        let mut sim_text = String::new();
        for ch in "speedtest".chars() {
            let s = ch.to_string();
            let act = ctx.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_text.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_text.push_str(commit);
            } else if act.action_type == 0 {
                sim_text.push(ch);
            }
        }
        assert_eq!(sim_text, "speedtest");

        let mut ctx2 = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx2.apply_config(&cfg);
        let mut sim_text2 = String::new();
        for ch in "different".chars() {
            let s = ch.to_string();
            let act = ctx2.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_text2.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_text2.push_str(commit);
            } else if act.action_type == 0 {
                sim_text2.push(ch);
            }
        }
        assert_eq!(sim_text2, "diferent");

        let mut ctx3 = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx3.apply_config(&cfg);
        let mut sim_text3 = String::new();
        for ch in "diffferent".chars() {
            let s = ch.to_string();
            let act = ctx3.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_text3.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_text3.push_str(commit);
            } else if act.action_type == 0 {
                sim_text3.push(ch);
            }
        }
        assert_eq!(sim_text3, "different");
    }
}
