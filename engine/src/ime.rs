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
pub const ACTION_AUTOCORRECT: i32 = 5;

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
    pub autocorrect: bool,
    pub last_committed: Option<(String, String, std::time::Instant)>,
    pub uncommit_candidate: Option<(String, String, std::time::Instant)>,
    pub pending_retype: Option<(String, String)>,
    in_identifier: bool,
    current_cursor: usize,
    pub last_replace_time: Option<std::time::Instant>,
    pub last_replace_cursor: Option<usize>,
    pub max_surr_lag_ms: u64,
    pub max_surr_window_chars: usize,
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
            autocorrect: false,
            last_committed: None,
            uncommit_candidate: None,
            pending_retype: None,
            in_identifier: false,
            current_cursor: 0,
            last_replace_time: None,
            last_replace_cursor: None,
            max_surr_lag_ms: 200,
            max_surr_window_chars: 8,
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
        self.engine
            .set_auto_restore_english(cfg.spelling.auto_restore_english);
        self.engine.set_dict(cfg.spelling.enabled);
        self.engine.set_short_w(cfg.general.short_w);
        self.bracket_brackets = cfg.general.bracket_brackets;
        self.double_space_period = cfg.typing.double_space_period;
        self.auto_capitalize = cfg.typing.auto_capitalize;
        self.autocorrect = cfg.spelling.autocorrect;
        self.debug_log = cfg.advanced.debug_log;
        self.macros_enabled = cfg.macros.enabled;
        self.macros = cfg
            .macros
            .items
            .iter()
            .map(|item| (item.trigger.clone(), item.replace.clone()))
            .collect();
        self.max_surr_lag_ms = cfg.pacing.max_surr_lag_ms;
        self.max_surr_window_chars = cfg.pacing.max_surr_window_chars;
    }

    pub fn reset(&mut self) {
        self.raw_buffer.clear();
        self.last_composed.clear();
        self.prev_composed.clear();
        self.typed_over_selection = false;
        self.delete_buf = CString::default();
        self.last_surr = None;
        self.in_identifier = false;
        self.last_replace_time = None;
        self.last_replace_cursor = None;
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
        self.current_cursor = cursor;
        if has_ctrl_alt {
            self.reset();
            self.uncommit_candidate = None;
            self.just_deleted = false;
            self.stale_surr = None;
            self.sentence_cap_state = SentenceCapState::Normal;
            return self.forward();
        }

        if key_sym == 0xff08 {
            self.sentence_cap_state = SentenceCapState::Normal;

            // pass through backspace inside identifier token
            if self.in_identifier {
                self.just_deleted = true;
                if !self.raw_buffer.is_empty() {
                    self.raw_buffer.pop();
                    self.last_composed = self.raw_buffer.clone();
                }
                if self.raw_buffer.is_empty() {
                    self.in_identifier = false;
                }
                return self.forward();
            }

            // check if backspacing over trailing space to uncommit and resume previous word
            if let Some((cand_raw, cand_com, cand_time)) = self.uncommit_candidate.take() {
                if cand_time.elapsed().as_secs() <= 10 {
                    let mut matched = false;
                    if let Some(text) = surrounding_text {
                        let chars: Vec<char> = text.chars().collect();
                        let cur = std::cmp::min(chars.len(), cursor);
                        if cur > 0 && chars[cur - 1] == ' ' {
                            let before_sp: String = chars[..cur - 1].iter().collect();
                            if before_sp.ends_with(&cand_com) {
                                matched = true;
                            }
                        }
                    } else {
                        matched = true;
                    }

                    if matched {
                        self.raw_buffer = cand_raw;
                        self.last_composed = cand_com;
                        self.just_deleted = false;
                        self.stale_surr = None;
                        return self.forward();
                    }
                }
            }

            self.just_deleted = true;

            // detect backspace after committed word for adaptive learning
            if let Some((last_raw, last_com, last_time)) = &self.last_committed {
                if last_time.elapsed().as_secs() <= 4 && self.raw_buffer.is_empty() {
                    self.pending_retype = Some((last_raw.clone(), last_com.clone()));
                }
            }

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
            self.uncommit_candidate = None;
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
            self.uncommit_candidate = None;
            self.just_deleted = false;
            self.stale_surr = None;
            self.pending_retype = None;
            self.last_committed = None;
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
            let cur_raw = self.raw_buffer.clone();
            let cur_com = if !self.last_composed.is_empty() {
                self.last_composed.clone()
            } else {
                self.raw_buffer.clone()
            };

            // adaptive learning: observe retype if user previously backspaced a word
            if let Some((old_raw, _old_com)) = self.pending_retype.take() {
                if !cur_com.is_empty() {
                    crate::learned_typos::observe_retype(&old_raw, &cur_com);
                }
            }

            if !cur_raw.is_empty() {
                self.last_committed =
                    Some((cur_raw.clone(), cur_com.clone(), std::time::Instant::now()));
            }

            if key_sym == 0x20 && !cur_raw.is_empty() && !cur_com.is_empty() {
                self.uncommit_candidate = Some((cur_raw, cur_com, std::time::Instant::now()));
            } else {
                self.uncommit_candidate = None;
            }

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
                    self.uncommit_candidate = None;
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
                        self.uncommit_candidate = None;
                        self.reset();
                        self.just_deleted = false;
                        self.stale_surr = None;
                        return self.replace(1, " ", ". ");
                    }
                }
            }

            // priority 1: autocorrect scrambled telex words at word break (telex only)
            if self.autocorrect
                && self.engine.method() == Method::Telex
                && !self.raw_buffer.is_empty()
                && (self.last_composed == self.raw_buffer
                    || !crate::spelling::is_valid_cvc(&self.last_composed))
            {
                if let Some(corrected) = crate::autocorrect::try_autocorrect_telex(
                    &self.raw_buffer,
                    self.engine.modern(),
                    self.engine.short_w(),
                ) {
                    let del_count = self.last_composed.chars().count();
                    let mut orig_text = self.last_composed.clone();
                    orig_text.push_str(key_str);
                    let mut commit_text = corrected;
                    commit_text.push_str(key_str);
                    self.uncommit_candidate = None;
                    self.reset();
                    self.just_deleted = false;
                    self.stale_surr = None;
                    self.commit_buf = CString::new(commit_text).unwrap_or_default();
                    self.delete_buf = CString::new(orig_text).unwrap_or_default();
                    return ImeAction {
                        action_type: ACTION_AUTOCORRECT,
                        delete_count: del_count,
                        commit_str: self.commit_buf.as_ptr(),
                        delete_str: self.delete_buf.as_ptr(),
                    };
                }
            }

            // priority 2: auto-restore incomplete stop coda words
            if self.engine.auto_restore()
                && !self.last_composed.is_empty()
                && self.last_composed != self.raw_buffer
                && crate::spelling::is_incomplete_stop_coda(&self.last_composed)
            {
                let del_count = self.last_composed.chars().count();
                let del_str = self.last_composed.clone();
                let mut commit_text = self.raw_buffer.clone();
                commit_text.push_str(key_str);
                self.uncommit_candidate = None;
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

        self.uncommit_candidate = None;
        self.just_deleted = false;

        let surr_meta = surrounding_text.map(|text| {
            let mut char_count = 0;
            let mut cur_byte = text.len();
            let mut anch_byte = text.len();
            for (idx, _) in text.char_indices() {
                if char_count == cursor {
                    cur_byte = idx;
                }
                if char_count == anchor {
                    anch_byte = idx;
                }
                char_count += 1;
            }
            (cur_byte, anch_byte, char_count)
        });

        let mut has_autocomplete = false;
        if let Some(text) = surrounding_text {
            let (cur_byte, anch_byte, char_count) = surr_meta.unwrap();
            let is_stale = match &self.stale_surr {
                Some((st_text, st_cur)) => st_text == text && *st_cur == cursor,
                None => false,
            };
            self.stale_surr = None;

            let sel_start_byte = std::cmp::min(cur_byte, anch_byte);
            let sel_end_byte = std::cmp::max(cur_byte, anch_byte);
            let cur_clamped = std::cmp::min(cursor, char_count);
            let anch_clamped = std::cmp::min(anchor, char_count);
            let sel_start = std::cmp::min(cur_clamped, anch_clamped);
            let sel_end = std::cmp::max(cur_clamped, anch_clamped);

            // address bar autocomplete applies to single-line fields without newlines
            let is_single_line = !text.is_empty() && !text.contains('\n');

            // browser address bar autocomplete detection
            if is_single_line
                && !self.raw_buffer.is_empty()
                && sel_start_byte < sel_end_byte
                && sel_end_byte == text.len()
            {
                let before_sel = &text[..sel_start_byte];
                if before_sel.ends_with(&self.last_composed) {
                    has_autocomplete = true;
                }
            }

            if is_single_line && self.typed_over_selection && sel_start_byte < sel_end_byte {
                has_autocomplete = true;
            }

            let is_same_surr = match &self.last_surr {
                Some((prev_text, prev_cur)) => prev_text == text && *prev_cur == cursor,
                None => false,
            };

            let before_cur = &text[..cur_byte];
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

            if !matches_last && !self.raw_buffer.is_empty() && cur_byte > 0 {
                let word = if let Some(last_break_pos) =
                    before_cur.rfind(|c: char| is_word_break(c as u32))
                {
                    let ch_len = before_cur[last_break_pos..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    &before_cur[last_break_pos + ch_len..]
                } else {
                    before_cur
                };
                if !word.is_empty() {
                    let word_telex = decompose_to_telex(word);
                    if self.raw_buffer.starts_with(&word_telex) || self.raw_buffer.starts_with(word)
                    {
                        matches_last = true;
                    } else {
                        // match unaccented base only when replacement occurred recently in same word
                        let window_limit = std::cmp::max(
                            self.max_surr_window_chars,
                            self.raw_buffer.chars().count().saturating_add(2),
                        );
                        let is_stale_lag = match (self.last_replace_time, self.last_replace_cursor)
                        {
                            (Some(t), Some(c)) => {
                                t.elapsed().as_millis() <= self.max_surr_lag_ms as u128
                                    && cursor >= c
                                    && (cursor - c) <= window_limit
                            }
                            _ => false,
                        };
                        if is_stale_lag {
                            let word_base = to_base_ascii(word);
                            let composed_base = to_base_ascii(&self.last_composed);
                            if !word_base.is_empty()
                                && (composed_base == word_base
                                    || composed_base.starts_with(&word_base)
                                    || word_base.starts_with(&composed_base))
                            {
                                matches_last = true;
                            }
                        }
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
                if is_single_line && sel_start == 0 && sel_end == char_count {
                    self.typed_over_selection = true;
                }
            }

            // word seeding from surrounding text only when cursor is at word end
            let is_at_word_end = cur_byte == text.len()
                || text[cur_byte..]
                    .chars()
                    .next()
                    .map(|c| is_word_break(c as u32))
                    .unwrap_or(true);
            let is_telex_mod = matches!(
                key_ch.to_ascii_lowercase(),
                'a' | 'e' | 'o' | 'd' | 'w' | 's' | 'f' | 'r' | 'x' | 'j'
            );
            if !is_stale
                && !has_autocomplete
                && cursor == anchor
                && self.last_composed.is_empty()
                && cur_byte > 0
                && is_telex_mod
                && is_at_word_end
            {
                let old_word = if let Some(last_break_pos) =
                    before_cur.rfind(|c: char| is_word_break(c as u32))
                {
                    let ch_len = before_cur[last_break_pos..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    &before_cur[last_break_pos + ch_len..]
                } else {
                    before_cur
                };
                if !old_word.is_empty() && old_word.chars().all(|c| c.is_alphabetic()) {
                    self.raw_buffer = decompose_to_telex(old_word);
                    self.last_composed = old_word.to_string();
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
                if let (Some(text), Some((cur_byte, _, _))) = (surrounding_text, surr_meta) {
                    let before_cur = &text[..cur_byte];
                    if before_cur.is_empty() {
                        should_cap = true;
                    } else {
                        let trimmed = before_cur.trim_end_matches(char::is_whitespace);
                        if trimmed.len() < before_cur.len() && !trimmed.is_empty() {
                            if let Some(last_char) = trimmed.chars().next_back() {
                                if matches!(last_char, '.' | '?' | '!') {
                                    should_cap = true;
                                }
                            }
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

        // protect file extensions and code identifiers after boundaries
        if self.raw_buffer.is_empty() && key_ch.is_alphabetic() {
            if let (Some(text), Some((cur_byte, _, _))) = (surrounding_text, surr_meta) {
                let before_cur = &text[..cur_byte];
                let mut prev_chars = before_cur.chars().rev();
                if let Some(prev_ch) = prev_chars.next() {
                    let is_ident_boundary = match prev_ch {
                        '_' | '/' => true,
                        '.' => prev_chars
                            .next()
                            .map(|c| !c.is_whitespace())
                            .unwrap_or(false),
                        '-' => prev_chars
                            .next()
                            .map(|c| c.is_alphabetic())
                            .unwrap_or(false),
                        _ => false,
                    };
                    if is_ident_boundary {
                        self.in_identifier = true;
                    }
                }
            }
        }

        if self.in_identifier {
            self.raw_buffer.push_str(key_str);
            self.last_composed = self.raw_buffer.clone();
            if let Some(text) = surrounding_text {
                self.last_surr = Some((text.to_string(), cursor));
            }
            return self.forward();
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
        self.last_replace_time = Some(std::time::Instant::now());
        self.last_replace_cursor = Some(self.current_cursor);
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

pub fn to_base_ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a' | 'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ'
            | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' => 'a',
            'A' | 'Á' | 'À' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ắ' | 'Ằ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Â' | 'Ấ'
            | 'Ầ' | 'Ẩ' | 'Ẫ' | 'Ậ' => 'A',
            'e' | 'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => {
                'e'
            }
            'E' | 'É' | 'È' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ế' | 'Ề' | 'Ể' | 'Ễ' | 'Ệ' => {
                'E'
            }
            'i' | 'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
            'I' | 'Í' | 'Ì' | 'Ỉ' | 'Ĩ' | 'Ị' => 'I',
            'o' | 'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ'
            | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
            'O' | 'Ó' | 'Ò' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ố' | 'Ồ' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ơ' | 'Ớ'
            | 'Ờ' | 'Ở' | 'Ỡ' | 'Ợ' => 'O',
            'u' | 'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => {
                'u'
            }
            'U' | 'Ú' | 'Ù' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ứ' | 'Ừ' | 'Ử' | 'Ữ' | 'Ự' => {
                'U'
            }
            'y' | 'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
            'Y' | 'Ý' | 'Ỳ' | 'Ỷ' | 'Ỹ' | 'Ỵ' => 'Y',
            'đ' => 'd',
            'Đ' => 'D',
            other => other,
        })
        .collect()
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
        c.uncommit_candidate = None;
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

#[no_mangle]
pub unsafe extern "C" fn clak_observe_retype(
    old_raw: *const c_char,
    new_word: *const c_char,
) -> bool {
    if old_raw.is_null() || new_word.is_null() {
        return false;
    }
    let r = match CStr::from_ptr(old_raw).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let w = match CStr::from_ptr(new_word).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    crate::learned_typos::observe_retype(r, w).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uncommit_on_backspace_after_space_resumes_word() {
        let mut ctx = ClakContext::new(Method::Telex);

        // type "toan"
        for ch in "toan".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        assert_eq!(ctx.last_composed, "toan");

        // type space
        let act = ctx.process_key(0x20, " ", false, Some("toan"), 4, 4);
        assert_eq!(act.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "");
        assert!(ctx.uncommit_candidate.is_some());

        // backspace space
        let act = ctx.process_key(0xff08, "", false, Some("toan "), 5, 5);
        assert_eq!(act.action_type, ACTION_FORWARD);
        assert_eq!(ctx.raw_buffer, "toan");
        assert_eq!(ctx.last_composed, "toan");

        // type 's' to add tone
        let act = ctx.process_key('s' as u32, "s", false, Some("toan"), 4, 4);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "toán");
    }

    #[test]
    fn test_uncommit_invalidated_when_new_word_started() {
        let mut ctx = ClakContext::new(Method::Telex);

        // type "toan" + space
        for ch in "toan".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        ctx.process_key(0x20, " ", false, Some("toan"), 4, 4);
        assert!(ctx.uncommit_candidate.is_some());

        // type 'k' (starts new word)
        ctx.process_key('k' as u32, "k", false, Some("toan "), 5, 5);
        assert!(ctx.uncommit_candidate.is_none());

        // backspace 'k'
        ctx.process_key(0xff08, "", false, Some("toan k"), 6, 6);
        assert!(ctx.uncommit_candidate.is_none());

        // backspace space
        ctx.process_key(0xff08, "", false, Some("toan "), 5, 5);
        // candidate was destroyed by typing 'k', must not resume
        assert_eq!(ctx.last_composed, "");
    }

    #[test]
    fn test_uncommit_invalidated_on_double_space() {
        let mut ctx = ClakContext::new(Method::Telex);

        for ch in "toan".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        // first space sets candidate
        ctx.process_key(0x20, " ", false, Some("toan"), 4, 4);
        assert!(ctx.uncommit_candidate.is_some());

        // second space invalidates candidate
        ctx.process_key(0x20, " ", false, Some("toan "), 5, 5);
        assert!(ctx.uncommit_candidate.is_none());
    }

    #[test]
    fn test_uncommit_vni_method() {
        let mut ctx = ClakContext::new(Method::Vni);

        for ch in "toan".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        ctx.process_key(0x20, " ", false, Some("toan"), 4, 4);

        // backspace space
        ctx.process_key(0xff08, "", false, Some("toan "), 5, 5);
        assert_eq!(ctx.last_composed, "toan");

        // type '1' for sắc tone
        let act = ctx.process_key('1' as u32, "1", false, Some("toan"), 4, 4);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "toán");
    }

    #[test]
    fn test_uncommit_resumed_without_surrounding_text() {
        let mut ctx = ClakContext::new(Method::Telex);

        for ch in "toan".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        ctx.process_key(0x20, " ", false, None, 0, 0);
        assert!(ctx.uncommit_candidate.is_some());

        // backspace in terminal without surrounding text resumes candidate for re-toning
        let act = ctx.process_key(0xff08, "", false, None, 0, 0);
        assert_eq!(act.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "toan");
        assert_eq!(ctx.raw_buffer, "toan");

        // typing tone key applies to resumed word
        let act = ctx.process_key('s' as u32, "s", false, None, 0, 0);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "toán");
    }

    #[test]
    fn test_uncommit_cleared_on_auto_restore_stop_coda() {
        let mut ctx = ClakContext::new(Method::Telex);

        for ch in "ddaattat".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        // space triggers auto-restore due to invalid stop coda
        let act = ctx.process_key(0x20, " ", false, None, 0, 0);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert!(ctx.uncommit_candidate.is_none());
        assert_eq!(ctx.raw_buffer, "");
    }
}
