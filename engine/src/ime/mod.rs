pub mod action;
pub mod decomposing;
pub mod dynamic_macro;
pub mod ffi;
pub mod surrounding;
pub mod typing_features;
pub mod uncommit;

pub use action::*;
pub use decomposing::*;
pub use dynamic_macro::*;
pub use ffi::*;
pub use surrounding::*;
pub use typing_features::*;
pub use uncommit::*;

use crate::{ClakCore, Method};
use std::ffi::CString;

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
    pub tab_macros: std::collections::HashMap<String, String>,
    pub colon_macros: std::collections::HashMap<String, String>,
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
    pub last_macro_expansion: Option<(String, String, std::time::Instant)>,
    pub last_autocorrect: Option<(String, String, std::time::Instant)>,
    dynamic_macros: DynamicMacroStore,
    recent_line: String,
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
            tab_macros: std::collections::HashMap::new(),
            colon_macros: std::collections::HashMap::new(),
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
            last_macro_expansion: None,
            last_autocorrect: None,
            dynamic_macros: DynamicMacroStore::default(),
            recent_line: String::new(),
        }
    }

    pub fn is_composing(&self) -> bool {
        !self.last_composed.is_empty() || !self.raw_buffer.is_empty()
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
        self.engine
            .set_allow_foreign_consonants(cfg.spelling.allow_foreign_consonants);
        self.engine.set_dict(cfg.spelling.enabled);
        self.engine.set_short_w(cfg.general.short_w);
        self.bracket_brackets = cfg.general.bracket_brackets;
        self.double_space_period = cfg.typing.double_space_period;
        self.auto_capitalize = cfg.typing.auto_capitalize;
        self.autocorrect = cfg.spelling.autocorrect;
        self.debug_log = cfg.advanced.debug_log;
        self.macros_enabled = cfg.macros.enabled;
        self.macros.clear();
        self.tab_macros.clear();
        self.colon_macros.clear();
        for item in &cfg.macros.items {
            let t = item.trigger.trim();
            if t.len() >= 3 && t.starts_with(':') && t.ends_with(':') {
                let inner = t[1..t.len() - 1].trim().to_lowercase();
                if !inner.is_empty() {
                    self.colon_macros.insert(inner, item.replace.clone());
                }
            } else if item.tab_only {
                self.tab_macros.insert(t.to_string(), item.replace.clone());
            } else {
                self.macros.insert(t.to_string(), item.replace.clone());
            }
        }
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
        self.last_macro_expansion = None;
        self.last_autocorrect = None;
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
            self.recent_line.clear();
            self.uncommit_candidate = None;
            self.just_deleted = false;
            self.stale_surr = None;
            self.sentence_cap_state = SentenceCapState::Normal;
            return self.forward();
        }

        // handle backspace
        if key_sym == 0xff08 {
            let act = self.handle_backspace(surrounding_text, cursor, anchor);
            if act.action_type == ACTION_FORWARD {
                self.pop_recent_line(1);
            }
            return act;
        }

        // any key other than backspace invalidates pending macro or autocorrect undo
        self.last_macro_expansion = None;
        self.last_autocorrect = None;

        // handle enter or tab
        if key_sym == 0xff0d || key_sym == 0xff09 {
            return self.handle_enter_or_tab(key_sym, surrounding_text, cursor);
        }

        // handle navigation and cancel keys
        if key_sym == 0xff1b
            || key_sym == 0xffff
            || key_sym == 0xff9f
            || (0xff50..=0xff57).contains(&key_sym)
        {
            self.reset();
            self.recent_line.clear();
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

        // bracket shortcuts
        if let Some(bracket_action) = check_bracket_shortcuts(
            self.bracket_brackets,
            &self.last_composed,
            &self.raw_buffer,
            key_str,
        ) {
            match bracket_action {
                BracketAction::UndoPair(del_str, target) => {
                    self.reset();
                    return self.replace(1, del_str, target);
                }
                BracketAction::Pair(target) => {
                    self.raw_buffer.push_str(key_str);
                    self.last_composed = target.to_string();
                    return self.replace(0, "", target);
                }
            }
        }

        let key_ch = key_str.chars().next().unwrap_or('\0');
        let act = if is_word_break(key_ch as u32) {
            self.handle_word_break(key_sym, key_str, key_ch, surrounding_text, cursor)
        } else {
            self.handle_printable(key_str, key_ch, surrounding_text, cursor, anchor)
        };
        if act.action_type == ACTION_FORWARD {
            self.push_recent_line(key_str);
        }
        act
    }

    fn handle_backspace(
        &mut self,
        surrounding_text: Option<&str>,
        cursor: usize,
        anchor: usize,
    ) -> ImeAction {
        self.sentence_cap_state = SentenceCapState::Normal;

        // undo macro expansion if backspaced immediately
        if let Some((raw_with_boundary, expanded_with_boundary, time)) =
            self.last_macro_expansion.take()
        {
            if time.elapsed().as_millis() <= 3000 {
                let matches_surr = match surrounding_text {
                    Some(text) => {
                        let chars: Vec<char> = text.chars().collect();
                        let cur = std::cmp::min(chars.len(), cursor);
                        let before_cur: String = chars[..cur].iter().collect();
                        before_cur.ends_with(&expanded_with_boundary)
                    }
                    None => true,
                };
                if matches_surr {
                    let del_count = expanded_with_boundary.chars().count();
                    self.reset();
                    self.just_deleted = true;
                    if raw_with_boundary.ends_with(' ') {
                        let trimmed = &raw_with_boundary[..raw_with_boundary.len() - 1];
                        self.uncommit_candidate = Some((
                            trimmed.to_string(),
                            trimmed.to_string(),
                            std::time::Instant::now(),
                        ));
                    }
                    return self.replace(del_count, &expanded_with_boundary, &raw_with_boundary);
                }
            }
        }

        // undo autocorrect if backspaced immediately
        if let Some((orig_with_boundary, corrected_with_boundary, time)) =
            self.last_autocorrect.take()
        {
            if time.elapsed().as_millis() <= 3000 {
                let matches_surr = match surrounding_text {
                    Some(text) => {
                        let chars: Vec<char> = text.chars().collect();
                        let cur = std::cmp::min(chars.len(), cursor);
                        let before_cur: String = chars[..cur].iter().collect();
                        before_cur.ends_with(&corrected_with_boundary)
                    }
                    None => true,
                };
                if matches_surr {
                    let del_count = corrected_with_boundary.chars().count();
                    self.reset();
                    self.just_deleted = true;
                    if orig_with_boundary.ends_with(' ') {
                        let trimmed = &orig_with_boundary[..orig_with_boundary.len() - 1];
                        self.uncommit_candidate = Some((
                            trimmed.to_string(),
                            trimmed.to_string(),
                            std::time::Instant::now(),
                        ));
                    }
                    return self.replace(del_count, &corrected_with_boundary, &orig_with_boundary);
                }
            }
        }

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
        if let Some((cand_raw, cand_com)) =
            try_match_uncommit(self.uncommit_candidate.take(), surrounding_text, cursor)
        {
            self.raw_buffer = cand_raw;
            self.last_composed = cand_com;
            self.just_deleted = true;
            self.last_replace_time = Some(std::time::Instant::now());
            self.last_replace_cursor = Some(cursor.saturating_sub(1));
            self.stale_surr = None;
            return self.forward();
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
            let mut popped_raw = false;
            if !self.raw_buffer.is_empty() {
                let mut raw_try = self.raw_buffer.clone();
                raw_try.pop();
                if self.engine.transform(&raw_try) == remaining {
                    self.raw_buffer = raw_try;
                    popped_raw = true;
                }
            }
            if !popped_raw {
                self.raw_buffer = decompose_to_raw(&remaining, self.engine.method());
            }
            self.last_composed = remaining;
            if let Some(text) = surrounding_text {
                self.last_surr = Some((text.to_string(), cursor));
            }
        }
        self.forward()
    }

    fn try_expand_colon_macro(
        &mut self,
        surrounding_text: Option<&str>,
        cursor: usize,
        trailing: &str,
    ) -> Option<ImeAction> {
        if !self.macros_enabled {
            return None;
        }
        let surr_before: &str = match surrounding_text {
            Some(text) => {
                let (cur_byte, _, _) = parse_surrounding_meta(text, cursor, cursor);
                &text[..cur_byte]
            }
            None => "",
        };
        let before_cursor = if !surr_before.is_empty() {
            surr_before
        } else {
            self.recent_line.as_str()
        };
        let (full_token, inner) = extract_colon_macro_at_end(before_cursor)?;
        let replacement = self.dynamic_macros.resolve(inner, &self.colon_macros)?;
        let del_count = full_token.chars().count();
        let del_str = full_token.to_string();
        let mut commit_text = replacement;
        commit_text.push_str(trailing);
        let mut raw_with_boundary = del_str.clone();
        raw_with_boundary.push_str(trailing);
        self.reset();
        self.last_macro_expansion = Some((
            raw_with_boundary,
            commit_text.clone(),
            std::time::Instant::now(),
        ));
        self.last_autocorrect = None;
        self.just_deleted = false;
        self.stale_surr = None;
        Some(self.replace(del_count, &del_str, &commit_text))
    }

    fn handle_enter_or_tab(
        &mut self,
        key_sym: u32,
        surrounding_text: Option<&str>,
        cursor: usize,
    ) -> ImeAction {
        self.uncommit_candidate = None;
        self.sentence_cap_state = SentenceCapState::Normal;

        // check :dynamic macro: expansion on Tab before regular single-word macros
        if key_sym == 0xff09 && self.macros_enabled {
            if let Some(act) = self.try_expand_colon_macro(surrounding_text, cursor, "") {
                return act;
            }

            // check user macros configured with tab_only = true (expanded without trailing tab)
            if !self.last_composed.is_empty() && !self.tab_macros.is_empty() {
                let tab_repl = match_macro(&self.tab_macros, &self.last_composed)
                    .or_else(|| match_macro(&self.tab_macros, &self.raw_buffer));
                if let Some(replacement) = tab_repl {
                    let del_count = self.last_composed.chars().count();
                    let del_str = self.last_composed.clone();
                    self.reset();
                    self.last_macro_expansion = Some((
                        del_str.clone(),
                        replacement.clone(),
                        std::time::Instant::now(),
                    ));
                    self.last_autocorrect = None;
                    self.just_deleted = false;
                    self.stale_surr = None;
                    return self.replace(del_count, &del_str, &replacement);
                }
            }
        }

        // check macro expansion on enter or tab
        if self.macros_enabled && !self.last_composed.is_empty() {
            if let Some(replacement) = self.lookup_macro() {
                let del_count = self.last_composed.chars().count();
                let del_str = self.last_composed.clone();
                let mut commit_text = replacement;
                let boundary = if key_sym == 0xff09 { "\t" } else { "\n" };
                commit_text.push_str(boundary);
                let mut raw_with_boundary = del_str.clone();
                raw_with_boundary.push_str(boundary);
                self.uncommit_candidate = None;
                self.reset();
                self.last_macro_expansion = Some((
                    raw_with_boundary,
                    commit_text.clone(),
                    std::time::Instant::now(),
                ));
                self.last_autocorrect = None;
                self.just_deleted = false;
                self.stale_surr = None;
                return self.replace(del_count, &del_str, &commit_text);
            }
        }

        if self.engine.auto_restore()
            && !self.last_composed.is_empty()
            && self.last_composed != self.raw_buffer
            && (crate::spelling::is_incomplete_stop_coda(&self.last_composed)
                || crate::is_open_short_vowel_with_onset(&self.last_composed))
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
        self.recent_line.clear();
        self.just_deleted = false;
        self.stale_surr = None;
        self.forward()
    }

    fn handle_word_break(
        &mut self,
        key_sym: u32,
        key_str: &str,
        key_ch: char,
        surrounding_text: Option<&str>,
        cursor: usize,
    ) -> ImeAction {
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
                let mut raw_with_boundary = del_str.clone();
                raw_with_boundary.push_str(key_str);
                self.uncommit_candidate = None;
                self.reset();
                self.last_macro_expansion = Some((
                    raw_with_boundary,
                    commit_text.clone(),
                    std::time::Instant::now(),
                ));
                self.last_autocorrect = None;
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
                self.last_autocorrect = Some((
                    orig_text.clone(),
                    commit_text.clone(),
                    std::time::Instant::now(),
                ));
                self.last_macro_expansion = None;
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
            && (crate::spelling::is_incomplete_stop_coda(&self.last_composed)
                || crate::is_open_short_vowel_with_onset(&self.last_composed))
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
        self.forward()
    }

    fn handle_printable(
        &mut self,
        key_str: &str,
        key_ch: char,
        surrounding_text: Option<&str>,
        cursor: usize,
        anchor: usize,
    ) -> ImeAction {
        self.uncommit_candidate = None;
        self.just_deleted = false;

        let surr_meta = surrounding_text.map(|text| parse_surrounding_meta(text, cursor, anchor));

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

            let effective_cur_byte = if has_autocomplete {
                sel_start_byte
            } else {
                cur_byte
            };
            let effective_cursor = if has_autocomplete { sel_start } else { cursor };
            let before_cur = &text[..effective_cur_byte];
            let word = extract_word_trimmed(before_cur);

            let matches_last = matches_last_composed_or_prefix(
                before_cur,
                word,
                &self.last_composed,
                &self.prev_composed,
                &self.raw_buffer,
                self.engine.method(),
                self.last_replace_time,
                self.last_replace_cursor,
                effective_cursor,
                self.max_surr_lag_ms,
                self.max_surr_window_chars,
            );

            if sel_start == sel_end {
                self.typed_over_selection = false;
                if !is_stale && !is_same_surr && !self.last_composed.is_empty() && !matches_last {
                    self.reset();
                }
            } else if !has_autocomplete {
                self.reset();
                if is_single_line && sel_start == 0 && sel_end == char_count {
                    self.typed_over_selection = true;
                }
            } else if !self.typed_over_selection && !matches_last && !self.last_composed.is_empty()
            {
                self.reset();
            }

            // word seeding from surrounding text before cursor
            let is_seed_key = is_modifier_for_method(self.engine.method(), key_ch);
            let right_is_ident_sep = is_right_ident_separator(&text[cur_byte..]);
            let can_seed_autocomplete =
                has_autocomplete && is_single_line && sel_start == cursor && sel_end == char_count;
            if !is_stale
                && (!has_autocomplete || can_seed_autocomplete)
                && (cursor == anchor || can_seed_autocomplete)
                && self.last_composed.is_empty()
                && cur_byte > 0
                && is_seed_key
                && !right_is_ident_sep
            {
                let old_word = extract_word_immediate(before_cur);
                if !old_word.is_empty() && old_word.chars().all(|c| c.is_alphabetic()) {
                    let seeded_raw = decompose_to_raw(old_word, self.engine.method());
                    let mut test_buf = seeded_raw.clone();
                    test_buf.push_str(key_str);
                    let transformed = self.engine.transform(&test_buf);
                    let (del, added) = compare_and_split(old_word, &transformed);
                    let degrades_to_raw = old_word != seeded_raw && transformed == test_buf;
                    let emits_corrupt_seq = del.chars().count() > 0
                        && (added.contains("ootj")
                            || added.contains("moot")
                            || added.contains("otoj"));
                    let old_was_accented = !old_word.is_ascii();
                    let destroys_accent = old_was_accented
                        && del.chars().count() > 0
                        && (!crate::spelling::is_valid_cvc(&transformed)
                            || transformed == test_buf);
                    if !degrades_to_raw && !emits_corrupt_seq && !destroys_accent {
                        self.raw_buffer = seeded_raw;
                        self.last_composed = old_word.to_string();
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

        if added_part.contains("ootj")
            || added_part.contains("moot")
            || added_part.contains("otoj")
            || added_part.contains("motoj")
        {
            self.reset();
            return self.forward();
        }

        if deleted_part.is_empty() && added_part == key_str {
            self.prev_composed = std::mem::replace(&mut self.last_composed, new_word);
            if let Some(text) = surrounding_text {
                self.last_surr = Some((text.to_string(), cursor));
            }
            return self.forward();
        }

        // do not swallow a doubled literal modifier when screen word already ends with that ascii char
        if deleted_part.is_empty()
            && added_part.is_empty()
            && self.last_composed.is_ascii()
            && self
                .last_composed
                .to_ascii_lowercase()
                .ends_with(&key_str_to_use.to_ascii_lowercase())
        {
            let mut next_composed = self.last_composed.clone();
            next_composed.push_str(key_str_to_use);
            self.raw_buffer.push_str(key_str_to_use);
            self.prev_composed = std::mem::replace(&mut self.last_composed, next_composed);
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

    fn push_recent_line(&mut self, text: &str) {
        self.recent_line.push_str(text);
        let char_len = self.recent_line.chars().count();
        if char_len > 160 {
            let drop_chars = char_len - 160;
            if let Some((byte_idx, _)) = self.recent_line.char_indices().nth(drop_chars) {
                self.recent_line.drain(..byte_idx);
            }
        }
    }

    fn pop_recent_line(&mut self, count: usize) {
        for _ in 0..count {
            self.recent_line.pop();
        }
    }

    fn replace(&mut self, delete_count: usize, deleted_str: &str, text: &str) -> ImeAction {
        self.pop_recent_line(delete_count);
        self.push_recent_line(text);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uncommit_on_backspace_after_space_resumes_word() {
        let mut ctx = ClakContext::new(Method::Telex);

        for ch in "toan".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        assert_eq!(ctx.last_composed, "toan");

        let act = ctx.process_key(0x20, " ", false, Some("toan"), 4, 4);
        assert_eq!(act.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "");
        assert!(ctx.uncommit_candidate.is_some());

        let act = ctx.process_key(0xff08, "", false, Some("toan "), 5, 5);
        assert_eq!(act.action_type, ACTION_FORWARD);
        assert_eq!(ctx.raw_buffer, "toan");
        assert_eq!(ctx.last_composed, "toan");

        let act = ctx.process_key('s' as u32, "s", false, Some("toan"), 4, 4);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert_eq!(ctx.last_composed, "toán");
    }

    #[test]
    fn test_uncommit_invalidated_when_new_word_started() {
        let mut ctx = ClakContext::new(Method::Telex);

        for ch in "toan".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        ctx.process_key(0x20, " ", false, Some("toan"), 4, 4);
        assert!(ctx.uncommit_candidate.is_some());

        ctx.process_key('k' as u32, "k", false, Some("toan "), 5, 5);
        assert!(ctx.uncommit_candidate.is_none());

        ctx.process_key(0xff08, "", false, Some("toan k"), 6, 6);
        assert!(ctx.uncommit_candidate.is_none());

        ctx.process_key(0xff08, "", false, Some("toan "), 5, 5);
        assert_eq!(ctx.last_composed, "");
    }

    #[test]
    fn test_uncommit_invalidated_on_double_space() {
        let mut ctx = ClakContext::new(Method::Telex);

        for ch in "toan".chars() {
            ctx.process_key(ch as u32, &ch.to_string(), false, None, 0, 0);
        }
        ctx.process_key(0x20, " ", false, Some("toan"), 4, 4);
        assert!(ctx.uncommit_candidate.is_some());

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

        ctx.process_key(0xff08, "", false, Some("toan "), 5, 5);
        assert_eq!(ctx.last_composed, "toan");

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

        let act = ctx.process_key(0xff08, "", false, None, 0, 0);
        assert_eq!(act.action_type, ACTION_FORWARD);
        assert_eq!(ctx.last_composed, "toan");
        assert_eq!(ctx.raw_buffer, "toan");

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
        let act = ctx.process_key(0x20, " ", false, None, 0, 0);
        assert_eq!(act.action_type, ACTION_REPLACE);
        assert!(ctx.uncommit_candidate.is_none());
        assert_eq!(ctx.raw_buffer, "");
    }
}
