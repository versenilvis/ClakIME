use crate::{ClakCore, Method};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[repr(C)]
pub struct ImeAction {
    pub action_type: i32,
    pub delete_count: usize,
    pub commit_str: *const c_char,
    pub delete_str: *const c_char,
    pub delete_offset: i32,
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
    pub raw_buffer: String,
    pub last_composed: String,
    prev_composed: String,
    stale_surr: Option<(String, usize)>,
    just_deleted: bool,
    typed_over_selection: bool,
    bracket_brackets: bool,
    double_space_period: bool,
    auto_capitalize: bool,
    sentence_cap_state: SentenceCapState,
    macros_enabled: bool,
    pub macros: std::collections::HashMap<String, String>,
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
            if self.engine.auto_restore()
                && !self.last_composed.is_empty()
                && self.last_composed != self.raw_buffer
                && crate::spelling::is_incomplete_stop_coda(&self.last_composed)
            {
                let del_count = self.last_composed.chars().count();
                let del_str = self.last_composed.clone();
                let mut commit_text = self.raw_buffer.clone();
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

            if self.engine.auto_restore()
                && !self.last_composed.is_empty()
                && self.last_composed != self.raw_buffer
                && crate::spelling::is_incomplete_stop_coda(&self.last_composed)
            {
                let del_count = self.last_composed.chars().count();
                let del_str = self.last_composed.clone();
                let mut commit_text = self.raw_buffer.clone();
                commit_text.push_str(key_str);
                self.reset();
                self.just_deleted = false;
                self.stale_surr = None;
                return self.replace(del_count, &del_str, &commit_text);
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
            let mut matches_last = !self.last_composed.is_empty()
                && (before_cur.ends_with(&self.last_composed)
                    || (!self.prev_composed.is_empty()
                        && before_cur.ends_with(&self.prev_composed))
                    || (!self.raw_buffer.is_empty() && before_cur.ends_with(&self.raw_buffer)));

            if !matches_last && !self.last_composed.is_empty() {
                for i in (1..self.last_composed.len()).rev() {
                    if let Some(prefix) = self.last_composed.get(..i) {
                        if !prefix.is_empty() && before_cur.ends_with(prefix) {
                            matches_last = true;
                            break;
                        }
                    }
                }
            }

            if !matches_last && !self.prev_composed.is_empty() {
                for i in (1..self.prev_composed.len()).rev() {
                    if let Some(prefix) = self.prev_composed.get(..i) {
                        if !prefix.is_empty() && before_cur.ends_with(prefix) {
                            matches_last = true;
                            break;
                        }
                    }
                }
            }

            if !matches_last && !self.raw_buffer.is_empty() {
                for i in (1..self.raw_buffer.len()).rev() {
                    if let Some(prefix) = self.raw_buffer.get(..i) {
                        if !prefix.is_empty() && before_cur.ends_with(prefix) {
                            matches_last = true;
                            break;
                        }
                    }
                }
            }

            if !matches_last && !self.raw_buffer.is_empty() && cur > 0 {
                let mut start = cur;
                while start > 0 && !is_word_break(chars[start - 1] as u32) {
                    start -= 1;
                }
                let word: String = chars[start..cur].iter().collect();
                if !word.is_empty() {
                    let word_telex = decompose_to_telex(&word);
                    if self.raw_buffer.starts_with(&word_telex)
                        || self.raw_buffer.starts_with(&word)
                    {
                        matches_last = true;
                    }
                }
            }

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

            let is_telex_mod = matches!(
                key_ch.to_ascii_lowercase(),
                'a' | 'e' | 'o' | 'd' | 'w' | 's' | 'f' | 'r' | 'x' | 'j' | 'z'
            );
            let is_vni_mod = matches!(key_ch, '0'..='9');
            let is_modifier = match self.engine.method() {
                Method::Vni | Method::TeipVni => is_vni_mod,
                _ => is_telex_mod,
            };

            if !just_del
                && !is_stale
                && !has_autocomplete
                && cursor == anchor
                && self.last_composed.is_empty()
                && is_modifier
            {
                let mut start = cur;
                while start > 0 {
                    let prev_char = chars[start - 1];
                    if is_word_break(prev_char as u32) {
                        break;
                    }
                    start -= 1;
                }
                let mut end = cur;
                while end < chars.len() {
                    let next_char = chars[end];
                    if is_word_break(next_char as u32) {
                        break;
                    }
                    end += 1;
                }

                if start < end {
                    let target_word: String = chars[start..end].iter().collect();
                    if !target_word.is_empty() && target_word.chars().all(|c| c.is_alphabetic()) {
                        let before_chars = cur - start;
                        let after_chars = end - cur;
                        let total_chars = end - start;

                        if after_chars == 0 {
                            // word-end seeding for standard pipeline
                            self.raw_buffer = match self.engine.method() {
                                Method::Vni | Method::TeipVni => decompose_to_vni(&target_word),
                                _ => decompose_to_telex(&target_word),
                            };
                            self.last_composed = target_word;
                        } else {
                            // mid-word or word-start retone
                            let raw = match self.engine.method() {
                                Method::Vni | Method::TeipVni => decompose_to_vni(&target_word),
                                _ => decompose_to_telex(&target_word),
                            };
                            let mut cand_raw = raw;
                            cand_raw.push_str(key_str);
                            let new_word = self.engine.transform(&cand_raw);

                            if new_word != target_word
                                && (crate::spelling::is_valid_cvc(&new_word)
                                    || (self.engine.auto_restore()
                                        && self.engine.dict_known(&new_word.to_lowercase())))
                            {
                                self.raw_buffer = cand_raw;
                                self.last_composed = new_word.clone();
                                self.prev_composed = target_word.clone();
                                if let Some(text) = surrounding_text {
                                    self.last_surr = Some((text.to_string(), cursor));
                                }
                                let delete_offset = -(before_chars as i32);
                                let delete_count = total_chars;
                                return self.replace_surrounding(
                                    delete_offset,
                                    delete_count,
                                    &target_word,
                                    &new_word,
                                );
                            }
                        }
                    }
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
            delete_offset: 0,
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
            delete_offset: -(delete_count as i32),
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
            delete_offset: -(delete_count as i32),
        }
    }

    fn replace_surrounding(
        &mut self,
        delete_offset: i32,
        delete_count: usize,
        deleted_str: &str,
        text: &str,
    ) -> ImeAction {
        self.commit_buf = CString::new(text).unwrap_or_default();
        self.delete_buf = CString::new(deleted_str).unwrap_or_default();
        ImeAction {
            action_type: ACTION_REPLACE_SURROUNDING,
            delete_count,
            commit_str: self.commit_buf.as_ptr(),
            delete_str: self.delete_buf.as_ptr(),
            delete_offset,
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

fn decompose_char_to_vni(c: char) -> (&'static str, Option<char>) {
    match c {
        'á' => ("a", Some('1')),
        'à' => ("a", Some('2')),
        'ả' => ("a", Some('3')),
        'ã' => ("a", Some('4')),
        'ạ' => ("a", Some('5')),
        'Á' => ("A", Some('1')),
        'À' => ("A", Some('2')),
        'Ả' => ("A", Some('3')),
        'Ã' => ("A", Some('4')),
        'Ạ' => ("A", Some('5')),
        'â' => ("a6", None),
        'ấ' => ("a6", Some('1')),
        'ầ' => ("a6", Some('2')),
        'ẩ' => ("a6", Some('3')),
        'ẫ' => ("a6", Some('4')),
        'ậ' => ("a6", Some('5')),
        'Â' => ("A6", None),
        'Ấ' => ("A6", Some('1')),
        'Ầ' => ("A6", Some('2')),
        'Ẩ' => ("A6", Some('3')),
        'Ẫ' => ("A6", Some('4')),
        'Ậ' => ("A6", Some('5')),
        'ă' => ("a8", None),
        'ắ' => ("a8", Some('1')),
        'ằ' => ("a8", Some('2')),
        'ẳ' => ("a8", Some('3')),
        'ẵ' => ("a8", Some('4')),
        'ặ' => ("a8", Some('5')),
        'Ă' => ("A8", None),
        'Ắ' => ("A8", Some('1')),
        'Ằ' => ("A8", Some('2')),
        'Ẳ' => ("A8", Some('3')),
        'Ẵ' => ("A8", Some('4')),
        'Ặ' => ("A8", Some('5')),
        'é' => ("e", Some('1')),
        'è' => ("e", Some('2')),
        'ẻ' => ("e", Some('3')),
        'ẽ' => ("e", Some('4')),
        'ẹ' => ("e", Some('5')),
        'É' => ("E", Some('1')),
        'È' => ("E", Some('2')),
        'Ẻ' => ("E", Some('3')),
        'Ẽ' => ("E", Some('4')),
        'Ẹ' => ("E", Some('5')),
        'ê' => ("e6", None),
        'ế' => ("e6", Some('1')),
        'ề' => ("e6", Some('2')),
        'ể' => ("e6", Some('3')),
        'ễ' => ("e6", Some('4')),
        'ệ' => ("e6", Some('5')),
        'Ê' => ("E6", None),
        'Ế' => ("E6", Some('1')),
        'Ề' => ("E6", Some('2')),
        'Ể' => ("E6", Some('3')),
        'Ễ' => ("E6", Some('4')),
        'Ệ' => ("E6", Some('5')),
        'í' => ("i", Some('1')),
        'ì' => ("i", Some('2')),
        'ỉ' => ("i", Some('3')),
        'ĩ' => ("i", Some('4')),
        'ị' => ("i", Some('5')),
        'Í' => ("I", Some('1')),
        'Ì' => ("I", Some('2')),
        'Ỉ' => ("I", Some('3')),
        'Ĩ' => ("I", Some('4')),
        'Ị' => ("I", Some('5')),
        'ó' => ("o", Some('1')),
        'ò' => ("o", Some('2')),
        'ỏ' => ("o", Some('3')),
        'õ' => ("o", Some('4')),
        'ọ' => ("o", Some('5')),
        'Ó' => ("O", Some('1')),
        'Ò' => ("O", Some('2')),
        'Ỏ' => ("O", Some('3')),
        'Õ' => ("O", Some('4')),
        'Ọ' => ("O", Some('5')),
        'ô' => ("o6", None),
        'ố' => ("o6", Some('1')),
        'ồ' => ("o6", Some('2')),
        'ổ' => ("o6", Some('3')),
        'ỗ' => ("o6", Some('4')),
        'ộ' => ("o6", Some('5')),
        'Ô' => ("O6", None),
        'Ố' => ("O6", Some('1')),
        'Ồ' => ("O6", Some('2')),
        'Ổ' => ("O6", Some('3')),
        'Ỗ' => ("O6", Some('4')),
        'Ộ' => ("O6", Some('5')),
        'ơ' => ("o7", None),
        'ớ' => ("o7", Some('1')),
        'ờ' => ("o7", Some('2')),
        'ở' => ("o7", Some('3')),
        'ỡ' => ("o7", Some('4')),
        'ợ' => ("o7", Some('5')),
        'Ơ' => ("O7", None),
        'Ớ' => ("O7", Some('1')),
        'Ờ' => ("O7", Some('2')),
        'Ở' => ("O7", Some('3')),
        'Ỡ' => ("O7", Some('4')),
        'Ợ' => ("O7", Some('5')),
        'ú' => ("u", Some('1')),
        'ù' => ("u", Some('2')),
        'ủ' => ("u", Some('3')),
        'ũ' => ("u", Some('4')),
        'ụ' => ("u", Some('5')),
        'Ú' => ("U", Some('1')),
        'Ù' => ("U", Some('2')),
        'Ủ' => ("U", Some('3')),
        'Ũ' => ("U", Some('4')),
        'Ụ' => ("U", Some('5')),
        'ư' => ("u7", None),
        'ứ' => ("u7", Some('1')),
        'ừ' => ("u7", Some('2')),
        'ử' => ("u7", Some('3')),
        'ữ' => ("u7", Some('4')),
        'ự' => ("u7", Some('5')),
        'Ư' => ("U7", None),
        'Ứ' => ("U7", Some('1')),
        'Ừ' => ("U7", Some('2')),
        'Ử' => ("U7", Some('3')),
        'Ữ' => ("U7", Some('4')),
        'Ự' => ("U7", Some('5')),
        'ý' => ("y", Some('1')),
        'ỳ' => ("y", Some('2')),
        'ỷ' => ("y", Some('3')),
        'ỹ' => ("y", Some('4')),
        'ỵ' => ("y", Some('5')),
        'Ý' => ("Y", Some('1')),
        'Ỳ' => ("Y", Some('2')),
        'Ỷ' => ("Y", Some('3')),
        'Ỹ' => ("Y", Some('4')),
        'Ỵ' => ("Y", Some('5')),
        'đ' => ("d9", None),
        'Đ' => ("D9", None),
        _ => ("", None),
    }
}

pub fn decompose_to_vni(word: &str) -> String {
    let mut result = String::with_capacity(word.len() + 4);
    let mut trailing_tone: Option<char> = None;

    for ch in word.chars() {
        let (base, tone) = decompose_char_to_vni(ch);
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
                    delete_offset: 0,
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
            delete_offset: 0,
        },
    }
}
