use crate::{ClakCore, Method};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[repr(C)]
pub struct ImeAction {
    pub action_type: i32,
    pub delete_count: usize,
    pub commit_str: *const c_char,
    pub delete_str: *const c_char,
}

pub const ACTION_FORWARD: i32 = 0;
pub const ACTION_COMMIT: i32 = 1;
pub const ACTION_REPLACE_SURROUNDING: i32 = 2;
pub const ACTION_ADDRESS_BAR_FIX: i32 = 3;
pub const ACTION_REPLACE: i32 = 4;
pub const ACTION_UINPUT_REPLACE: i32 = 4;

pub struct ClakContext {
    engine: ClakCore,
    commit_buf: CString,
    delete_buf: CString,
    raw_buffer: String,
    last_composed: String,
    prev_composed: String,
    stale_surr: Option<(String, usize)>,
    just_deleted: bool,
    typed_over_selection: bool,
    bracket_brackets: bool,
    double_space_period: bool,
    auto_capitalize: bool,
    sentence_cap_state: SentenceCapState,
    macros_enabled: bool,
    macros: std::collections::HashMap<String, String>,
    debug_log: bool,
    last_surr: Option<(String, usize)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SentenceCapState {
    StartOfInput,
    SawPunctuation,
    ReadyToCapitalize,
    Normal,
}

impl ClakContext {
    pub fn new(method: Method) -> Self {
        let mut engine = ClakCore::new(method);
        engine.set_auto_restore(true);
        engine.set_dict(false);
        Self {
            engine,
            commit_buf: CString::default(),
            delete_buf: CString::default(),
            raw_buffer: String::new(),
            last_composed: String::new(),
            prev_composed: String::new(),
            stale_surr: None,
            just_deleted: false,
            typed_over_selection: false,
            bracket_brackets: true,
            double_space_period: false,
            auto_capitalize: false,
            sentence_cap_state: SentenceCapState::StartOfInput,
            macros_enabled: true,
            macros: std::collections::HashMap::new(),
            debug_log: false,
            last_surr: None,
        }
    }

    pub fn apply_config(&mut self, cfg: &crate::config::ClakConfig) {
        let m = match cfg.general.method.as_str() {
            "vni" => Method::Vni,
            "viqr" => Method::Viqr,
            "teip_vni" => Method::TeipVni,
            _ => Method::Telex,
        };
        self.engine.set_method(m);
        self.engine.set_modern(cfg.spelling.modern_tone);
        self.engine.set_auto_restore(cfg.spelling.auto_restore);
        self.engine.set_dict(cfg.spelling.enabled);
        self.engine.set_short_w(cfg.general.short_w);
        self.bracket_brackets = cfg.general.bracket_brackets;
        self.double_space_period = cfg.typing.double_space_period;
        self.auto_capitalize = cfg.typing.auto_capitalize;
        self.debug_log = cfg.advanced.debug_log;
        self.macros_enabled = cfg.macros.enabled;
        self.macros = cfg
            .macros
            .items
            .iter()
            .map(|item| (item.trigger.clone(), item.replace.clone()))
            .collect();
    }

    pub fn reset(&mut self) {
        self.raw_buffer.clear();
        self.last_composed.clear();
        self.prev_composed.clear();
        self.typed_over_selection = false;
        self.delete_buf = CString::default();
        self.last_surr = None;
    }

    pub fn process_key(
        &mut self,
        key_sym: u32,
        key_str: &str,
        has_ctrl_alt: bool,
        surrounding_text: Option<&str>,
        cursor: usize,
        anchor: usize,
    ) -> ImeAction {
        if has_ctrl_alt {
            self.reset();
            self.just_deleted = false;
            self.stale_surr = None;
            self.sentence_cap_state = SentenceCapState::Normal;
            return self.forward();
        }

        if key_sym == 0xff08 {
            self.just_deleted = true;
            self.sentence_cap_state = SentenceCapState::Normal;
            if let Some(text) = surrounding_text {
                self.stale_surr = Some((text.to_string(), cursor));
                let chars: Vec<char> = text.chars().collect();
                let cur = std::cmp::min(chars.len(), cursor);
                let anch = std::cmp::min(chars.len(), anchor);
                if text.is_empty() || std::cmp::min(cur, anch) == 0 {
                    self.reset();
                    return self.forward();
                }
            } else {
                self.stale_surr = None;
            }

            if cursor != anchor {
                self.reset();
                return self.forward();
            }

            // reset state when deleting single char so no partial buffer is left
            if self.last_composed.chars().count() <= 1 {
                self.reset();
            } else {
                let mut chars: Vec<char> = self.last_composed.chars().collect();
                chars.pop();
                let remaining: String = chars.into_iter().collect();
                self.raw_buffer = decompose_to_telex(&remaining);
                self.last_composed = remaining;
                if let Some(text) = surrounding_text {
                    self.last_surr = Some((text.to_string(), cursor));
                }
            }
            return self.forward();
        }

        if key_sym == 0xff0d || key_sym == 0xff09 {
            self.sentence_cap_state = SentenceCapState::Normal;
            // check macro expansion on enter or tab
            if self.macros_enabled && !self.last_composed.is_empty() {
                if let Some(replacement) = self.lookup_macro() {
                    let del_count = self.last_composed.chars().count();
                    let del_str = self.last_composed.clone();
                    let mut commit_text = replacement;
                    if key_sym == 0xff09 {
                        commit_text.push('\t');
                    } else {
                        commit_text.push('\n');
                    }
                    self.reset();
                    self.just_deleted = false;
                    self.stale_surr = None;
                    return self.replace(del_count, &del_str, &commit_text);
                }
            }
            self.reset();
            self.just_deleted = false;
            self.stale_surr = None;
            return self.forward();
        }

        if key_sym == 0xff1b
            || key_sym == 0xffff
            || key_sym == 0xff9f
            || (0xff50..=0xff57).contains(&key_sym)
        {
            self.reset();
            self.just_deleted = false;
            self.stale_surr = None;
            self.sentence_cap_state = SentenceCapState::Normal;
            return self.forward();
        }

        if key_str.is_empty() {
            return self.forward();
        }

        // bracket shortcut handling for square and curly brackets
        if self.bracket_brackets {
            if self.last_composed == "ơ" && key_str == "[" {
                self.reset();
                return self.replace(1, "ơ", "[");
            } else if self.last_composed == "ư" && key_str == "]" {
                self.reset();
                return self.replace(1, "ư", "]");
            } else if self.last_composed == "Ơ" && key_str == "{" {
                self.reset();
                return self.replace(1, "Ơ", "{");
            } else if self.last_composed == "Ư" && key_str == "}" {
                self.reset();
                return self.replace(1, "Ư", "}");
            } else if self.raw_buffer.is_empty() {
                let mapped = match key_str {
                    "[" => Some("ơ"),
                    "]" => Some("ư"),
                    "{" => Some("Ơ"),
                    "}" => Some("Ư"),
                    _ => None,
                };
                if let Some(target) = mapped {
                    self.raw_buffer.push_str(key_str);
                    self.last_composed = target.to_string();
                    return self.replace(0, "", target);
                }
            }
        }

        let key_ch = key_str.chars().next().unwrap_or('\0');
        if is_word_break(key_ch as u32) {
            if self.auto_capitalize {
                if key_ch == '.' || key_ch == '?' || key_ch == '!' {
                    self.sentence_cap_state = SentenceCapState::SawPunctuation;
                } else if key_ch == ' '
                    && (self.sentence_cap_state == SentenceCapState::SawPunctuation
                        || self.sentence_cap_state == SentenceCapState::ReadyToCapitalize)
                {
                    self.sentence_cap_state = SentenceCapState::ReadyToCapitalize;
                } else {
                    self.sentence_cap_state = SentenceCapState::Normal;
                }
            }
            // check macro expansion before resetting
            if self.macros_enabled && !self.last_composed.is_empty() {
                if let Some(replacement) = self.lookup_macro() {
                    let del_count = self.last_composed.chars().count();
                    let del_str = self.last_composed.clone();
                    let mut commit_text = replacement;
                    commit_text.push_str(key_str);
                    self.reset();
                    self.just_deleted = false;
                    self.stale_surr = None;
                    return self.replace(del_count, &del_str, &commit_text);
                }
            }

            // check double space to period
            if self.double_space_period && key_sym == 0x20 {
                if let Some(text) = surrounding_text {
                    let chars: Vec<char> = text.chars().collect();
                    if cursor > 0 && cursor <= chars.len() && chars[cursor - 1] == ' ' {
                        self.reset();
                        self.just_deleted = false;
                        self.stale_surr = None;
                        return self.replace(1, " ", ". ");
                    }
                }
            }

            self.reset();
            self.just_deleted = false;
            self.stale_surr = None;
            return self.forward();
        }

        let just_del = self.just_deleted;
        self.just_deleted = false;

        let mut has_autocomplete = false;
        if let Some(text) = surrounding_text {
            let is_stale = match &self.stale_surr {
                Some((st_text, st_cur)) => st_text == text && *st_cur == cursor,
                None => false,
            };
            self.stale_surr = None;

            let chars: Vec<char> = text.chars().collect();
            let cur = std::cmp::min(chars.len(), cursor);
            let anch = std::cmp::min(chars.len(), anchor);
            let sel_start = std::cmp::min(cur, anch);
            let sel_end = std::cmp::max(cur, anch);

            // address bar autocomplete applies to single-line fields without newlines
            let is_single_line = !text.is_empty() && !text.contains('\n');

            // browser address bar autocomplete detection
            if is_single_line
                && !self.raw_buffer.is_empty()
                && sel_start < sel_end
                && sel_end == chars.len()
            {
                let before_sel: String = chars[..sel_start].iter().collect();
                if before_sel.ends_with(&self.last_composed) {
                    has_autocomplete = true;
                }
            }

            if is_single_line && self.typed_over_selection && sel_start < sel_end {
                has_autocomplete = true;
            }

            let is_same_surr = match &self.last_surr {
                Some((prev_text, prev_cur)) => prev_text == text && *prev_cur == cursor,
                None => false,
            };

            let before_cur: String = chars[..cur].iter().collect();
            let matches_last = !self.last_composed.is_empty()
                && (before_cur.ends_with(&self.last_composed)
                    || (!self.prev_composed.is_empty()
                        && before_cur.ends_with(&self.prev_composed))
                    || (!self.raw_buffer.is_empty() && before_cur.ends_with(&self.raw_buffer)));

            if sel_start == sel_end {
                self.typed_over_selection = false;
                // if surrounding text changed and cursor no longer follows our composed word, reset composition
                if !is_stale && !is_same_surr && !self.last_composed.is_empty() && !matches_last {
                    self.reset();
                }
            } else if !has_autocomplete {
                self.reset();
                if is_single_line && sel_start == 0 && sel_end == chars.len() {
                    self.typed_over_selection = true;
                }
            }

            // word seeding from surrounding text only when cursor is at word end
            let is_at_word_end = cur == chars.len() || is_word_break(chars[cur] as u32);
            let is_telex_mod = matches!(
                key_ch,
                'a' | 'e' | 'o' | 'd' | 'w' | 's' | 'f' | 'r' | 'x' | 'j'
            );
            if !just_del
                && !is_stale
                && !has_autocomplete
                && cursor == anchor
                && self.last_composed.is_empty()
                && cur > 0
                && is_telex_mod
                && is_at_word_end
            {
                let mut start = cur;
                while start > 0 {
                    let prev_char = chars[start - 1];
                    if is_word_break(prev_char as u32) {
                        break;
                    }
                    start -= 1;
                }
                let old_word: String = chars[start..cur].iter().collect();
                if !old_word.is_empty() && old_word.chars().all(|c| c.is_alphabetic()) {
                    self.raw_buffer = decompose_to_telex(&old_word);
                    self.last_composed = old_word;
                }
            }
        } else {
            self.stale_surr = None;
        }

        let mut key_str_to_use = key_str;
        let cap_buf;
        if self.auto_capitalize && key_ch.is_alphabetic() && key_ch.is_lowercase() {
            let mut should_cap = self.sentence_cap_state == SentenceCapState::ReadyToCapitalize;
            if !should_cap {
                if let Some(text) = surrounding_text {
                    let chars: Vec<char> = text.chars().collect();
                    let cur = std::cmp::min(chars.len(), cursor);
                    if cur == 0 {
                        should_cap = true;
                    } else {
                        let mut i = cur;
                        let mut saw_sp = false;
                        while i > 0 && chars[i - 1].is_whitespace() {
                            saw_sp = true;
                            i -= 1;
                        }
                        if saw_sp && i > 0 && matches!(chars[i - 1], '.' | '?' | '!') {
                            should_cap = true;
                        }
                    }
                }
            }
            if should_cap {
                cap_buf = key_ch.to_uppercase().to_string();
                key_str_to_use = &cap_buf;
            }
            self.sentence_cap_state = SentenceCapState::Normal;
        } else if key_ch.is_alphabetic() {
            self.sentence_cap_state = SentenceCapState::Normal;
        }

        self.raw_buffer.push_str(key_str_to_use);
        let new_word = self.engine.transform(&self.raw_buffer);

        let (deleted_part, added_part) = compare_and_split(&self.last_composed, &new_word);

        if deleted_part.is_empty() && added_part == key_str {
            self.prev_composed = std::mem::replace(&mut self.last_composed, new_word);
            if let Some(text) = surrounding_text {
                self.last_surr = Some((text.to_string(), cursor));
            }
            return self.forward();
        }

        let chars_to_delete = deleted_part.chars().count();
        self.prev_composed = std::mem::replace(&mut self.last_composed, new_word);

        if let Some(text) = surrounding_text {
            self.last_surr = Some((text.to_string(), cursor));
        }

        if has_autocomplete {
            self.typed_over_selection = false;
            return self.address_bar_fix(chars_to_delete, &deleted_part, &added_part);
        }

        self.replace(chars_to_delete, &deleted_part, &added_part)
    }

    fn forward(&self) -> ImeAction {
        ImeAction {
            action_type: ACTION_FORWARD,
            delete_count: 0,
            commit_str: std::ptr::null(),
            delete_str: std::ptr::null(),
        }
    }

    fn address_bar_fix(&mut self, delete_count: usize, deleted_str: &str, text: &str) -> ImeAction {
        self.commit_buf = CString::new(text).unwrap_or_default();
        self.delete_buf = CString::new(deleted_str).unwrap_or_default();
        ImeAction {
            action_type: ACTION_ADDRESS_BAR_FIX,
            delete_count,
            commit_str: self.commit_buf.as_ptr(),
            delete_str: self.delete_buf.as_ptr(),
        }
    }

    fn lookup_macro(&self) -> Option<String> {
        if !self.macros_enabled {
            return None;
        }
        if !self.last_composed.is_empty() {
            if let Some(repl) = match_macro(&self.macros, &self.last_composed) {
                return Some(repl);
            }
        }
        if !self.raw_buffer.is_empty() && self.raw_buffer != self.last_composed {
            if let Some(repl) = match_macro(&self.macros, &self.raw_buffer) {
                return Some(repl);
            }
        }
        None
    }

    fn replace(&mut self, delete_count: usize, deleted_str: &str, text: &str) -> ImeAction {
        self.commit_buf = CString::new(text).unwrap_or_default();
        self.delete_buf = CString::new(deleted_str).unwrap_or_default();
        ImeAction {
            action_type: ACTION_REPLACE,
            delete_count,
            commit_str: self.commit_buf.as_ptr(),
            delete_str: self.delete_buf.as_ptr(),
        }
    }
}

fn match_macro(macros: &std::collections::HashMap<String, String>, target: &str) -> Option<String> {
    if target.is_empty() {
        return None;
    }

    // exact match
    if let Some(repl) = macros.get(target) {
        return Some(repl.clone());
    }

    // case insensitive match
    let target_lower = target.to_lowercase();
    for (trigger, repl) in macros {
        if trigger.to_lowercase() == target_lower {
            let is_all_upper = target
                .chars()
                .all(|c| !c.is_alphabetic() || c.is_uppercase());
            let is_capitalized = {
                let mut chars = target.chars();
                match chars.next() {
                    Some(first) => {
                        first.is_uppercase()
                            && chars.all(|c| !c.is_alphabetic() || c.is_lowercase())
                    }
                    None => false,
                }
            };

            if is_all_upper {
                return Some(repl.to_uppercase());
            } else if is_capitalized {
                let mut repl_chars = repl.chars();
                let formatted = match repl_chars.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + repl_chars.as_str(),
                    None => String::new(),
                };
                return Some(formatted);
            } else {
                return Some(repl.clone());
            }
        }
    }

    None
}

fn is_word_break(ucs4: u32) -> bool {
    ucs4 == ' ' as u32
        || ucs4 == '\t' as u32
        || ucs4 == '\n' as u32
        || ucs4 == '\r' as u32
        || ucs4 == 0
        || (58..=64).contains(&ucs4)
        || (33..=47).contains(&ucs4)
        || (91..=96).contains(&ucs4)
        || (123..=126).contains(&ucs4)
}

fn decompose_char_to_telex(c: char) -> (&'static str, Option<char>) {
    match c {
        'á' => ("a", Some('s')),
        'à' => ("a", Some('f')),
        'ả' => ("a", Some('r')),
        'ã' => ("a", Some('x')),
        'ạ' => ("a", Some('j')),
        'Á' => ("A", Some('s')),
        'À' => ("A", Some('f')),
        'Ả' => ("A", Some('r')),
        'Ã' => ("A", Some('x')),
        'Ạ' => ("A", Some('j')),
        'â' => ("aa", None),
        'ấ' => ("aa", Some('s')),
        'ầ' => ("aa", Some('f')),
        'ẩ' => ("aa", Some('r')),
        'ẫ' => ("aa", Some('x')),
        'ậ' => ("aa", Some('j')),
        'Â' => ("AA", None),
        'Ấ' => ("AA", Some('s')),
        'Ầ' => ("AA", Some('f')),
        'Ẩ' => ("AA", Some('r')),
        'Ẫ' => ("AA", Some('x')),
        'Ậ' => ("AA", Some('j')),
        'ă' => ("aw", None),
        'ắ' => ("aw", Some('s')),
        'ằ' => ("aw", Some('f')),
        'ẳ' => ("aw", Some('r')),
        'ẵ' => ("aw", Some('x')),
        'ặ' => ("aw", Some('j')),
        'Ă' => ("AW", None),
        'Ắ' => ("AW", Some('s')),
        'Ằ' => ("AW", Some('f')),
        'Ẳ' => ("AW", Some('r')),
        'Ẵ' => ("AW", Some('x')),
        'Ặ' => ("AW", Some('j')),
        'é' => ("e", Some('s')),
        'è' => ("e", Some('f')),
        'ẻ' => ("e", Some('r')),
        'ẽ' => ("e", Some('x')),
        'ẹ' => ("e", Some('j')),
        'É' => ("E", Some('s')),
        'È' => ("E", Some('f')),
        'Ẻ' => ("E", Some('r')),
        'Ẽ' => ("E", Some('x')),
        'Ẹ' => ("E", Some('j')),
        'ê' => ("ee", None),
        'ế' => ("ee", Some('s')),
        'ề' => ("ee", Some('f')),
        'ể' => ("ee", Some('r')),
        'ễ' => ("ee", Some('x')),
        'ệ' => ("ee", Some('j')),
        'Ê' => ("EE", None),
        'Ế' => ("EE", Some('s')),
        'Ề' => ("EE", Some('f')),
        'Ể' => ("EE", Some('r')),
        'Ễ' => ("EE", Some('x')),
        'Ệ' => ("EE", Some('j')),
        'í' => ("i", Some('s')),
        'ì' => ("i", Some('f')),
        'ỉ' => ("i", Some('r')),
        'ĩ' => ("i", Some('x')),
        'ị' => ("i", Some('j')),
        'Í' => ("I", Some('s')),
        'Ì' => ("I", Some('f')),
        'Ỉ' => ("I", Some('r')),
        'Ĩ' => ("I", Some('x')),
        'Ị' => ("I", Some('j')),
        'ó' => ("o", Some('s')),
        'ò' => ("o", Some('f')),
        'ỏ' => ("o", Some('r')),
        'õ' => ("o", Some('x')),
        'ọ' => ("o", Some('j')),
        'Ó' => ("O", Some('s')),
        'Ò' => ("O", Some('f')),
        'Ỏ' => ("O", Some('r')),
        'Õ' => ("O", Some('x')),
        'Ọ' => ("O", Some('j')),
        'ô' => ("oo", None),
        'ố' => ("oo", Some('s')),
        'ồ' => ("oo", Some('f')),
        'ổ' => ("oo", Some('r')),
        'ỗ' => ("oo", Some('x')),
        'ộ' => ("oo", Some('j')),
        'Ô' => ("OO", None),
        'Ố' => ("OO", Some('s')),
        'Ồ' => ("OO", Some('f')),
        'Ổ' => ("OO", Some('r')),
        'Ỗ' => ("OO", Some('x')),
        'Ộ' => ("OO", Some('j')),
        'ơ' => ("ow", None),
        'ớ' => ("ow", Some('s')),
        'ờ' => ("ow", Some('f')),
        'ở' => ("ow", Some('r')),
        'ỡ' => ("ow", Some('x')),
        'ợ' => ("ow", Some('j')),
        'Ơ' => ("OW", None),
        'Ớ' => ("OW", Some('s')),
        'Ờ' => ("OW", Some('f')),
        'Ở' => ("OW", Some('r')),
        'Ỡ' => ("OW", Some('x')),
        'Ợ' => ("OW", Some('j')),
        'ú' => ("u", Some('s')),
        'ù' => ("u", Some('f')),
        'ủ' => ("u", Some('r')),
        'ũ' => ("u", Some('x')),
        'ụ' => ("u", Some('j')),
        'Ú' => ("U", Some('s')),
        'Ù' => ("U", Some('f')),
        'Ủ' => ("U", Some('r')),
        'Ũ' => ("U", Some('x')),
        'Ụ' => ("U", Some('j')),
        'ư' => ("uw", None),
        'ứ' => ("uw", Some('s')),
        'ừ' => ("uw", Some('f')),
        'ử' => ("uw", Some('r')),
        'ữ' => ("uw", Some('x')),
        'ự' => ("uw", Some('j')),
        'Ư' => ("UW", None),
        'Ứ' => ("UW", Some('s')),
        'Ừ' => ("UW", Some('f')),
        'Ử' => ("UW", Some('r')),
        'Ữ' => ("UW", Some('x')),
        'Ự' => ("UW", Some('j')),
        'ý' => ("y", Some('s')),
        'ỳ' => ("y", Some('f')),
        'ỷ' => ("y", Some('r')),
        'ỹ' => ("y", Some('x')),
        'ỵ' => ("y", Some('j')),
        'Ý' => ("Y", Some('s')),
        'Ỳ' => ("Y", Some('f')),
        'Ỷ' => ("Y", Some('r')),
        'Ỹ' => ("Y", Some('x')),
        'Ỵ' => ("Y", Some('j')),
        'đ' => ("dd", None),
        'Đ' => ("Dd", None),
        _ => ("", None),
    }
}

pub fn decompose_to_telex(word: &str) -> String {
    let mut result = String::with_capacity(word.len() + 4);
    let mut trailing_tone: Option<char> = None;

    for ch in word.chars() {
        let (base, tone) = decompose_char_to_telex(ch);
        if !base.is_empty() {
            result.push_str(base);
            if tone.is_some() {
                trailing_tone = tone;
            }
        } else {
            result.push(ch);
        }
    }

    if let Some(t) = trailing_tone {
        result.push(t);
    }

    result
}

pub fn compare_and_split(a: &str, b: &str) -> (String, String) {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let mut prefix_len = 0;
    while prefix_len < a_chars.len()
        && prefix_len < b_chars.len()
        && a_chars[prefix_len] == b_chars[prefix_len]
    {
        prefix_len += 1;
    }

    let deleted: String = a_chars[prefix_len..].iter().collect();
    let added: String = b_chars[prefix_len..].iter().collect();
    (deleted, added)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_context_new(method: i32) -> *mut ClakContext {
    let m = match method {
        0 => Method::Telex,
        1 => Method::Vni,
        2 => Method::Viqr,
        3 => Method::TeipVni,
        _ => Method::Telex,
    };
    Box::into_raw(Box::new(ClakContext::new(m)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_context_free(ctx: *mut ClakContext) {
    if !ctx.is_null() {
        drop(Box::from_raw(ctx));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_is_composing(ctx: *const ClakContext) -> bool {
    if let Some(c) = ctx.as_ref() {
        !c.last_composed.is_empty() || !c.raw_buffer.is_empty()
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_context_reset(ctx: *mut ClakContext) {
    if let Some(c) = ctx.as_mut() {
        c.reset();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_context_apply_config(
    ctx: *mut ClakContext,
    cfg: *const crate::config::ClakConfig,
) {
    if let (Some(c), Some(config)) = (ctx.as_mut(), cfg.as_ref()) {
        c.apply_config(config);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_process_key(
    ctx: *mut ClakContext,
    key_sym: u32,
    key_str: *const c_char,
    has_ctrl_alt: bool,
    surrounding_text: *const c_char,
    cursor: usize,
    anchor: usize,
) -> ImeAction {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let c = match ctx.as_mut() {
            Some(ptr) => ptr,
            None => {
                return ImeAction {
                    action_type: ACTION_FORWARD,
                    delete_count: 0,
                    commit_str: std::ptr::null(),
                    delete_str: std::ptr::null(),
                }
            }
        };

        let k_str = if key_str.is_null() {
            ""
        } else {
            CStr::from_ptr(key_str).to_str().unwrap_or_default()
        };

        let s_text = if surrounding_text.is_null() {
            None
        } else {
            CStr::from_ptr(surrounding_text).to_str().ok()
        };

        let action = c.process_key(key_sym, k_str, has_ctrl_alt, s_text, cursor, anchor);

        let log_enabled = c.debug_log
            || std::env::var("CLAK_LOG")
                .map(|v| v != "0" && v != "false" && v != "off")
                .unwrap_or(false);
        if log_enabled {
            use std::io::Write;
            use std::os::unix::fs::MetadataExt;
            use std::os::unix::fs::OpenOptionsExt;

            let mut opts = std::fs::OpenOptions::new();
            opts.create(true)
                .append(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
            if let Ok(mut f) = opts.open("/tmp/clak.log") {
                if let Ok(meta) = f.metadata() {
                    if meta.uid() == unsafe { libc::getuid() } && meta.file_type().is_file() {
                        let act_name = match action.action_type {
                            0 => "FORWARD",
                            1 => "COMMIT",
                            2 => "REPLACE_SURR",
                            3 => "ADDR_BAR_FIX",
                            4 => "REPLACE",
                            _ => "UNKNOWN",
                        };
                        let commit = if action.commit_str.is_null() {
                            ""
                        } else {
                            CStr::from_ptr(action.commit_str).to_str().unwrap_or("")
                        };
                        let mut surr_preview = String::from("none");
                        if let Some(t) = s_text {
                            let preview: String = t.chars().take(25).collect();
                            surr_preview = format!("'{}'(c={},a={})", preview, cursor, anchor);
                        }
                        let _ = writeln!(
                            f,
                            "Key: '{}' (0x{:x}) | Surr: {} | Act: {} (del={}) -> Commit: '{}' | Raw: '{}' | Composed: '{}'",
                            k_str, key_sym, surr_preview, act_name, action.delete_count, commit, c.raw_buffer, c.last_composed
                        );
                    }
                }
            }
        }

        action
    }));

    match result {
        Ok(action) => action,
        Err(_) => ImeAction {
            action_type: ACTION_FORWARD,
            delete_count: 0,
            commit_str: std::ptr::null(),
            delete_str: std::ptr::null(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surrounding_dd_to_d_stroke() {
        let mut ctx = ClakContext::new(Method::Telex);
        let action = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
        assert_eq!(action.action_type, ACTION_UINPUT_REPLACE);
        assert_eq!(action.delete_count, 1);
        let commit_text = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
        assert_eq!(commit_text, "đ");
    }

    #[test]
    fn test_address_bar_autocomplete_selection() {
        // cursor=1, anchor=10: user typed 'd', autocomplete showed 'douyin.com'
        // we must NOT seed from surrounding text (that would double-count the 'd')
        // so raw_buffer = "d" only, transform = "d", no replacement → forward
        // the autocomplete fix happens on the second 'd': see test_address_bar_autocomplete_flow
        let mut ctx = ClakContext::new(Method::Telex);
        let action = ctx.process_key(b'd' as u32, "d", false, Some("douyin.com"), 1, 10);
        // engine did not seed from surrounding text, so 'd' alone stays as 'd' → forward
        assert_eq!(action.action_type, ACTION_FORWARD);
    }

    #[test]
    fn test_normal_english_key_forwards() {
        let mut ctx = ClakContext::new(Method::Telex);
        let action = ctx.process_key(b'k' as u32, "k", false, Some("hello "), 6, 6);
        assert_eq!(action.action_type, ACTION_FORWARD);
    }

    #[test]
    fn test_address_bar_stale_surrounding() {
        let mut ctx = ClakContext::new(Method::Telex);
        let act1 = ctx.process_key(b'd' as u32, "d", false, Some("\n"), 0, 0);
        assert_eq!(act1.action_type, ACTION_FORWARD);
        let act2 = ctx.process_key(b'd' as u32, "d", false, Some("\n"), 0, 0);
        assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);
        assert_eq!(act2.delete_count, 1);
        let commit = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
        assert_eq!(commit, "đ");
    }

    #[test]
    fn test_address_bar_autocomplete_flow() {
        let mut ctx = ClakContext::new(Method::Telex);
        let act1 = ctx.process_key(b'd' as u32, "d", false, Some("\n"), 0, 0);
        assert_eq!(act1.action_type, ACTION_FORWARD);
        let act2 = ctx.process_key(b'd' as u32, "d", false, Some("discord.com"), 1, 11);
        assert_eq!(act2.action_type, ACTION_ADDRESS_BAR_FIX);
        assert_eq!(act2.delete_count, 1);
        let commit = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
        assert_eq!(commit, "đ");
    }

    #[test]
    fn test_address_bar_typing_over_selected_url() {
        let mut ctx = ClakContext::new(Method::Telex);
        let act1 = ctx.process_key(
            b'd' as u32,
            "d",
            false,
            Some("https://www.facebook.com"),
            0,
            24,
        );
        assert_eq!(act1.action_type, ACTION_FORWARD);
        let act2 = ctx.process_key(
            b'd' as u32,
            "d",
            false,
            Some("https://www.facebook.com"),
            0,
            24,
        );
        assert_eq!(act2.action_type, ACTION_ADDRESS_BAR_FIX);
        assert_eq!(act2.delete_count, 1);
        let commit = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
        assert_eq!(commit, "đ");
    }

    #[test]
    fn test_address_bar_delete_and_retype() {
        let mut ctx = ClakContext::new(Method::Telex);
        let act1 = ctx.process_key(b'd' as u32, "d", false, Some("\n"), 0, 0);
        assert_eq!(act1.action_type, ACTION_FORWARD);
        let act2 = ctx.process_key(b'd' as u32, "d", false, Some("discord.com"), 1, 11);
        assert_eq!(act2.action_type, ACTION_ADDRESS_BAR_FIX);
        assert_eq!(act2.delete_count, 1);

        // dismiss autocomplete suggestion
        let act_bs1 = ctx.process_key(0xff08, "", false, Some("đangcap.vn"), 1, 10);
        assert_eq!(act_bs1.action_type, ACTION_FORWARD);

        // delete 'đ' character
        let act_bs2 = ctx.process_key(0xff08, "", false, Some("đ"), 1, 1);
        assert_eq!(act_bs2.action_type, ACTION_FORWARD);

        // address bar is empty, type 'd' again
        let act3 = ctx.process_key(b'd' as u32, "d", false, Some(""), 0, 0);
        // must forward 'd', not become 'đ'
        assert_eq!(act3.action_type, ACTION_FORWARD);
    }

    #[test]
    fn test_address_bar_autocomplete_with_spaces_query() {
        let mut ctx = ClakContext::new(Method::Telex);
        // user types 'c'
        let a1 = ctx.process_key(b'c' as u32, "c", false, Some(""), 0, 0);
        assert_eq!(a1.action_type, ACTION_FORWARD);

        // browser autocompletes 'claude.ai'(1..9), user types 'a'
        let a2 = ctx.process_key(b'a' as u32, "a", false, Some("claude.ai"), 1, 9);
        assert_eq!(a2.action_type, ACTION_FORWARD);

        // browser autocompletes search suggestion 'cach check hạn gg pro'(2..21), user types 'n'
        let a3 = ctx.process_key(
            b'n' as u32,
            "n",
            false,
            Some("cach check hạn gg pro"),
            2,
            21,
        );
        assert_eq!(a3.action_type, ACTION_FORWARD);
        assert_eq!(ctx.raw_buffer, "can");

        // browser replaced selection with 'can', user types 'h'
        let a4 = ctx.process_key(b'h' as u32, "h", false, Some("can"), 3, 3);
        assert_eq!(a4.action_type, ACTION_FORWARD);
        assert_eq!(ctx.raw_buffer, "canh");

        // browser autocompletes 'canh chua ca loc'(4..17), user types 's'
        let a5 = ctx.process_key(b's' as u32, "s", false, Some("canh chua ca loc"), 4, 17);
        assert_eq!(a5.action_type, ACTION_ADDRESS_BAR_FIX);
        let commit5 = unsafe { CStr::from_ptr(a5.commit_str).to_str().unwrap() };
        assert_eq!(commit5, "ánh");

        // also verify without autocomplete on 's'
        let mut ctx2 = ClakContext::new(Method::Telex);
        ctx2.process_key(b'c' as u32, "c", false, Some(""), 0, 0);
        ctx2.process_key(b'a' as u32, "a", false, Some("claude.ai"), 1, 9);
        ctx2.process_key(
            b'n' as u32,
            "n",
            false,
            Some("cach check hạn gg pro"),
            2,
            21,
        );
        ctx2.process_key(b'h' as u32, "h", false, Some("can"), 3, 3);
        let a5_no_auto = ctx2.process_key(b's' as u32, "s", false, Some("canh"), 4, 4);
        assert_eq!(a5_no_auto.action_type, ACTION_REPLACE);
        let commit_no_auto = unsafe { CStr::from_ptr(a5_no_auto.commit_str).to_str().unwrap() };
        assert_eq!(commit_no_auto, "ánh");
    }

    #[test]
    fn test_backspace_and_retype_with_stale_surrounding() {
        let mut ctx = ClakContext::new(Method::Telex);
        // step 1: type 'dd' -> 'đ'
        ctx.process_key(b'd' as u32, "d", false, Some(""), 0, 0);
        let act2 = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
        assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);

        // step 2: backspace deletes 'đ'
        let act_bs = ctx.process_key(0xff08, "", false, Some("đ"), 1, 1);
        assert_eq!(act_bs.action_type, ACTION_FORWARD);

        // step 3: retype 'd' while surrounding is still stale 'đ'(1, 1)
        let act3 = ctx.process_key(b'd' as u32, "d", false, Some("đ"), 1, 1);
        assert_eq!(act3.action_type, ACTION_FORWARD);
        assert_eq!(ctx.raw_buffer, "d");
        assert_eq!(ctx.last_composed, "d");

        // step 4: type second 'd' -> should become 'đ'
        let act4 = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
        assert_eq!(act4.action_type, ACTION_UINPUT_REPLACE);
        let commit4 = unsafe { CStr::from_ptr(act4.commit_str).to_str().unwrap() };
        assert_eq!(commit4, "đ");
        assert_eq!(ctx.raw_buffer, "dd");
        assert_eq!(ctx.last_composed, "đ");
    }

    #[test]
    fn test_user_scenario_dd_del_dddddd_del_dd() {
        let mut ctx = ClakContext::new(Method::Telex);
        // step 1: dd -> đ
        let _ = ctx.process_key(b'd' as u32, "d", false, Some(""), 0, 0);
        let _ = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
        assert_eq!(ctx.last_composed, "đ");
        assert_eq!(ctx.raw_buffer, "dd");

        // step 2: backspace đ
        let _ = ctx.process_key(0xff08, "", false, Some("đ"), 1, 1);
        assert_eq!(ctx.last_composed, "");
        assert_eq!(ctx.raw_buffer, "");

        // step 3: type dddddd
        for _ in "dddddd".chars() {
            let _ = ctx.process_key(b'd' as u32, "d", false, None, 0, 0);
        }
        assert_eq!(ctx.last_composed, "ddddd");
        assert_eq!(ctx.raw_buffer, "dddddd");

        // step 4: backspace all
        let count = ctx.last_composed.chars().count();
        for _ in 0..count {
            let _ = ctx.process_key(0xff08, "", false, None, 0, 0);
        }
        assert_eq!(ctx.last_composed, "");
        assert_eq!(ctx.raw_buffer, "");

        // step 5: type dd -> đ cleanly
        let a_d1 = ctx.process_key(b'd' as u32, "d", false, None, 0, 0);
        assert_eq!(a_d1.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "d");
        assert_eq!(ctx.raw_buffer, "d");

        let a_d2 = ctx.process_key(b'd' as u32, "d", false, None, 0, 0);
        assert_eq!(a_d2.action_type, ACTION_UINPUT_REPLACE);
        assert_eq!(ctx.last_composed, "đ");
        assert_eq!(ctx.raw_buffer, "dd");
    }

    #[test]
    fn test_surrounding_vietnamese_word_progression() {
        let mut ctx = ClakContext::new(Method::Telex);
        let action = ctx.process_key(b'j' as u32, "j", false, Some("viêt"), 4, 4);
        let commit_text = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
        assert_eq!(action.action_type, ACTION_UINPUT_REPLACE);
        assert_eq!(action.delete_count, 2);
        assert_eq!(commit_text, "ệt");
    }

    fn simulate_typing(input: &str) -> String {
        let mut ctx = ClakContext::new(Method::Telex);
        let mut doc = String::new();
        for ch in input.chars() {
            let key_str = ch.to_string();
            let cur = doc.chars().count();
            let action = ctx.process_key(ch as u32, &key_str, false, Some(&doc), cur, cur);
            match action.action_type {
                ACTION_FORWARD => {
                    doc.push(ch);
                }
                ACTION_COMMIT => {
                    let s = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
                    doc.push_str(s);
                }
                ACTION_REPLACE_SURROUNDING | ACTION_ADDRESS_BAR_FIX | ACTION_UINPUT_REPLACE => {
                    let del = action.delete_count;
                    let s = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
                    let mut doc_chars: Vec<char> = doc.chars().collect();
                    for _ in 0..del {
                        doc_chars.pop();
                    }
                    doc = doc_chars.into_iter().collect();
                    doc.push_str(s);
                }
                _ => {}
            }
        }
        doc
    }

    fn simulate_typing_fallback(input: &str) -> String {
        let mut ctx = ClakContext::new(Method::Telex);
        let mut doc = String::new();
        for ch in input.chars() {
            let key_str = ch.to_string();
            let action = ctx.process_key(ch as u32, &key_str, false, None, 0, 0);
            match action.action_type {
                ACTION_FORWARD => {
                    doc.push(ch);
                }
                ACTION_COMMIT => {
                    let s = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
                    doc.push_str(s);
                }
                ACTION_REPLACE_SURROUNDING | ACTION_ADDRESS_BAR_FIX | ACTION_UINPUT_REPLACE => {
                    let del = action.delete_count;
                    let s = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
                    let mut doc_chars: Vec<char> = doc.chars().collect();
                    for _ in 0..del {
                        doc_chars.pop();
                    }
                    doc = doc_chars.into_iter().collect();
                    doc.push_str(s);
                }
                _ => {}
            }
        }
        doc
    }

    #[test]
    fn test_full_vietnamese_typing() {
        assert_eq!(simulate_typing("dd"), "đ");
        assert_eq!(simulate_typing("tieengs"), "tiếng");
        assert_eq!(simulate_typing("vieetj"), "việt");
        assert_eq!(simulate_typing("chaof"), "chào");
        assert_eq!(simulate_typing("nguwowif"), "người");
        assert_eq!(simulate_typing("dduwowcj"), "được");
        assert_eq!(simulate_typing("khoong"), "không");
        assert_eq!(
            simulate_typing("chaof banj tooi laf nguwowif vieetj nam"),
            "chào bạn tôi là người việt nam"
        );
        assert_eq!(simulate_typing("test"), "tét");
        assert_eq!(simulate_typing("assssssssss"), "asssssssss");
    }

    #[test]
    fn test_fallback_vietnamese_typing() {
        assert_eq!(simulate_typing_fallback("dd"), "đ");
        assert_eq!(simulate_typing_fallback("tieengs"), "tiếng");
        assert_eq!(simulate_typing_fallback("vieetj"), "việt");
        assert_eq!(simulate_typing_fallback("chaof"), "chào");
        assert_eq!(simulate_typing_fallback("nguwowif"), "người");
        assert_eq!(simulate_typing_fallback("dduwowcj"), "được");
        assert_eq!(simulate_typing_fallback("khoong"), "không");
        assert_eq!(simulate_typing_fallback("ddaay"), "đây");
        assert_eq!(simulate_typing_fallback("booj"), "bộ");
        assert_eq!(simulate_typing_fallback("mooj"), "mộ");
        assert_eq!(
            simulate_typing_fallback("chaof banj tooi laf nguwowif vieetj nam"),
            "chào bạn tôi là người việt nam"
        );
        assert_eq!(simulate_typing_fallback("test"), "tét");
        assert_eq!(simulate_typing_fallback("tesst"), "test");
        assert_eq!(simulate_typing_fallback("as"), "á");
        assert_eq!(simulate_typing_fallback("ass"), "as");
        assert_eq!(simulate_typing_fallback("asss"), "ass");
        assert_eq!(simulate_typing_fallback("assss"), "asss");
        assert_eq!(simulate_typing_fallback("assssssssss"), "asssssssss");
    }

    #[test]
    fn test_chuns_progression() {
        let mut ctx = ClakContext::new(Method::Telex);
        let keys = ["c", "h", "u", "n", "s"];
        for k in keys {
            ctx.process_key(k.chars().next().unwrap() as u32, k, false, None, 0, 0);
        }
        assert_eq!(ctx.last_composed, "chún");
        ctx.process_key('g' as u32, "g", false, None, 0, 0);
        assert_eq!(ctx.last_composed, "chúng");
    }

    #[test]
    fn test_twitter_placeholder_typing_is() {
        let mut ctx = ClakContext::new(Method::Telex);
        // Twitter/Facebook modal opens with placeholder or button text in surrounding
        let act1 = ctx.process_key(b'i' as u32, "i", false, Some("Có gì đang xảy ra?\n"), 0, 0);
        assert_eq!(act1.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "i");

        let act2 = ctx.process_key(b's' as u32, "s", false, Some("i"), 1, 1);
        assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);
        assert_eq!(act2.delete_count, 1);
        let commit2 = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
        assert_eq!(commit2, "í");

        // type second s to toggle to english 'is'
        let act3 = ctx.process_key(b's' as u32, "s", false, Some("í"), 1, 1);
        assert_eq!(act3.action_type, ACTION_UINPUT_REPLACE);
        assert_eq!(act3.delete_count, 1);
        let commit3 = unsafe { CStr::from_ptr(act3.commit_str).to_str().unwrap() };
        assert_eq!(commit3, "is");
    }

    #[test]
    fn test_multiline_editor_no_false_autocomplete() {
        let mut ctx = ClakContext::new(Method::Telex);
        // multiline editor with selection (cursor != anchor) should NOT trigger address bar autocomplete
        let act = ctx.process_key(b'i' as u32, "i", false, Some("Line 1\nLine 2"), 0, 6);
        assert_eq!(act.action_type, ACTION_FORWARD);
    }

    #[test]
    fn test_twitter_draftjs_trailing_newline_is() {
        let mut ctx = ClakContext::new(Method::Telex);
        // simulate Twitter Draft.js composer where empty box has '\n' and typed word has trailing '\n'
        let act1 = ctx.process_key(b'i' as u32, "i", false, Some("\n"), 0, 0);
        assert_eq!(act1.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "i");

        // key 's' arrives with surrounding "i\n" and cursor=1
        let act2 = ctx.process_key(b's' as u32, "s", false, Some("i\n"), 1, 1);
        assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);
        assert_eq!(act2.delete_count, 1);
        let commit2 = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
        assert_eq!(commit2, "í");
    }

    #[test]
    fn test_twitter_draftjs_reseed_uaj_to_ua_with_dot() {
        let mut ctx = ClakContext::new(Method::Telex);
        // even if reset happened between u and a:
        ctx.reset();
        // user presses 'a' with surrounding "u\n"
        let act1 = ctx.process_key(b'a' as u32, "a", false, Some("u\n"), 1, 1);
        assert_eq!(act1.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "ua");

        // user presses 'j' with surrounding "ua\n"
        let act2 = ctx.process_key(b'j' as u32, "j", false, Some("ua\n"), 2, 2);
        assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);
        assert_eq!(act2.delete_count, 2);
        let commit2 = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
        assert_eq!(commit2, "ụa");
    }

    #[test]
    fn test_typing_over_selected_word_in_sentence() {
        let mut ctx = ClakContext::new(Method::Telex);
        let text = "Fcitx5 and Wayland";
        // user selects "and" (indices 7..10) and types "và" (v, a, f)
        let a1 = ctx.process_key(b'v' as u32, "v", false, Some(text), 7, 10);
        assert_eq!(a1.action_type, ACTION_FORWARD);

        // editor replaced "and" with "v", cursor at 8, anchor at 8
        let a2 = ctx.process_key(b'a' as u32, "a", false, Some("Fcitx5 v Wayland"), 8, 8);
        assert_eq!(a2.action_type, ACTION_FORWARD);

        // typing 'f' to transform "va" -> "và"
        let a3 = ctx.process_key(b'f' as u32, "f", false, Some("Fcitx5 va Wayland"), 9, 9);
        assert_eq!(a3.action_type, ACTION_REPLACE);
        assert_eq!(a3.delete_count, 1);
        let commit3 = unsafe { CStr::from_ptr(a3.commit_str).to_str().unwrap() };
        assert_eq!(commit3, "à");
    }

    #[test]
    fn test_bracket_shortcuts() {
        let mut ctx = ClakContext::new(Method::Telex);
        let a1 = ctx.process_key(b'[' as u32, "[", false, None, 0, 0);
        assert_eq!(a1.action_type, ACTION_REPLACE);
        let commit1 = unsafe { CStr::from_ptr(a1.commit_str).to_str().unwrap() };
        assert_eq!(commit1, "ơ");

        let a2 = ctx.process_key(b'[' as u32, "[", false, None, 0, 0);
        assert_eq!(a2.action_type, ACTION_REPLACE);
        assert_eq!(a2.delete_count, 1);
        let commit2 = unsafe { CStr::from_ptr(a2.commit_str).to_str().unwrap() };
        assert_eq!(commit2, "[");
    }

    #[test]
    fn test_macro_expansion() {
        let mut ctx = ClakContext::new(Method::Telex);
        ctx.macros.insert("vn".to_string(), "Việt Nam".to_string());
        ctx.process_key(b'v' as u32, "v", false, None, 0, 0);
        ctx.process_key(b'n' as u32, "n", false, None, 1, 1);
        let act = ctx.process_key(0x20, " ", false, None, 2, 2);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(act.delete_count, 2);
        let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
        assert_eq!(commit, "Việt Nam ");
    }

    #[test]
    fn test_macro_case_uppercase() {
        let mut ctx = ClakContext::new(Method::Telex);
        ctx.macros.insert("vn".to_string(), "Việt Nam".to_string());
        ctx.process_key(b'V' as u32, "V", false, None, 0, 0);
        ctx.process_key(b'N' as u32, "N", false, None, 1, 1);
        let act = ctx.process_key(0x20, " ", false, None, 2, 2);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(act.delete_count, 2);
        let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
        assert_eq!(commit, "VIỆT NAM ");
    }

    #[test]
    fn test_macro_case_capitalized() {
        let mut ctx = ClakContext::new(Method::Telex);
        ctx.macros.insert("ko".to_string(), "không".to_string());
        ctx.process_key(b'K' as u32, "K", false, None, 0, 0);
        ctx.process_key(b'o' as u32, "o", false, None, 1, 1);
        let act = ctx.process_key(0x20, " ", false, None, 2, 2);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(act.delete_count, 2);
        let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
        assert_eq!(commit, "Không ");
    }

    #[test]
    fn test_macro_enter_trigger() {
        let mut ctx = ClakContext::new(Method::Telex);
        ctx.macros.insert("vn".to_string(), "Việt Nam".to_string());
        ctx.process_key(b'v' as u32, "v", false, None, 0, 0);
        ctx.process_key(b'n' as u32, "n", false, None, 1, 1);
        let act = ctx.process_key(0xff0d, "\n", false, None, 2, 2);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(act.delete_count, 2);
        let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
        assert_eq!(commit, "Việt Nam\n");
    }

    #[test]
    fn test_macro_raw_buffer_trigger() {
        let mut ctx = ClakContext::new(Method::Telex);
        ctx.macros.insert("ddc".to_string(), "được".to_string());
        ctx.process_key(b'd' as u32, "d", false, None, 0, 0);
        ctx.process_key(b'd' as u32, "d", false, None, 1, 1);
        ctx.process_key(b'c' as u32, "c", false, None, 2, 2);
        let act = ctx.process_key(0x20, " ", false, None, 2, 2);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(act.delete_count, 2);
        let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
        assert_eq!(commit, "được ");
    }

    #[test]
    fn test_delete_all_and_retype() {
        let mut ctx = ClakContext::new(Method::Telex);
        // step 1: type "in" -> forwards 'i', forwards 'n'
        ctx.process_key(b'i' as u32, "i", false, Some("\n"), 0, 0);
        ctx.process_key(b'n' as u32, "n", false, Some("i\n"), 1, 1);
        // step 2: type 's' -> replaces to "ín"
        let act_s = ctx.process_key(b's' as u32, "s", false, Some("in\n"), 2, 2);
        assert_eq!(act_s.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "ín");

        // step 3: user deletes everything in the editor!
        // editor text is now "\n", cursor at 0
        // user types 'i' again
        let act_i = ctx.process_key(b'i' as u32, "i", false, Some("\n"), 0, 0);
        assert_eq!(act_i.action_type, ACTION_FORWARD);
        assert_eq!(ctx.raw_buffer, "i");
        assert_eq!(ctx.last_composed, "i");

        // user types 'n'
        let act_n = ctx.process_key(b'n' as u32, "n", false, Some("i\n"), 1, 1);
        assert_eq!(act_n.action_type, ACTION_FORWARD);
        assert_eq!(ctx.raw_buffer, "in");
        assert_eq!(ctx.last_composed, "in");

        // user types 's'
        let act_s2 = ctx.process_key(b's' as u32, "s", false, Some("in\n"), 2, 2);
        assert_eq!(act_s2.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "ín");
    }

    #[test]
    fn test_is_composing_state() {
        let mut ctx = ClakContext::new(Method::Telex);
        assert!(!unsafe { clak_is_composing(&ctx as *const ClakContext) });

        ctx.process_key(b't' as u32, "t", false, None, 0, 0);
        assert!(unsafe { clak_is_composing(&ctx as *const ClakContext) });

        ctx.reset();
        assert!(!unsafe { clak_is_composing(&ctx as *const ClakContext) });
    }

    #[test]
    fn test_rapid_typing_lagging_surrounding_dduowcj() {
        let mut ctx = ClakContext::new(Method::Telex);
        // key 1: 'd' -> forwards 'd'
        let act1 = ctx.process_key(b'd' as u32, "d", false, Some(""), 0, 0);
        assert_eq!(act1.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "d");

        // key 2: 'd' -> replaces to 'đ'
        let act2 = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
        assert_eq!(act2.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "đ");

        // key 3: 'u' arrives before app processed replace, app still reports "d" at cursor 1
        let act3 = ctx.process_key(b'u' as u32, "u", false, Some("d"), 1, 1);
        assert_eq!(act3.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "đu");

        // key 4: 'o' -> app caught up to "đu" at cursor 2
        let act4 = ctx.process_key(b'o' as u32, "o", false, Some("đu"), 2, 2);
        assert_eq!(act4.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "đuo");

        // key 5: 'w' -> replaces to "đươ"
        let act5 = ctx.process_key(b'w' as u32, "w", false, Some("đuo"), 3, 3);
        assert_eq!(act5.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "đươ");

        // key 6: 'c' arrives before app processed replace, app still reports "đuo" at cursor 3
        let act6 = ctx.process_key(b'c' as u32, "c", false, Some("đuo"), 3, 3);
        assert_eq!(act6.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "đươc");

        // key 7: 'j' -> replaces to "được"
        let act7 = ctx.process_key(b'j' as u32, "j", false, Some("đươc"), 4, 4);
        assert_eq!(act7.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "được");
    }
}
