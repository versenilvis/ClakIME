use crate::ime::decomposing::{decompose_to_raw, to_base_ascii};
use crate::ime::typing_features::{is_word_break, is_zero_width};
use crate::Method;

pub fn parse_surrounding_meta(text: &str, cursor: usize, anchor: usize) -> (usize, usize, usize) {
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
}

pub fn extract_word_trimmed(before_cur: &str) -> &str {
    let trimmed = before_cur.trim_end_matches(|c: char| c == ' ' || is_zero_width(c));
    let trimmed = trimmed.trim_start_matches(is_zero_width);
    if let Some(last_break_pos) = trimmed.rfind(|c: char| is_word_break(c as u32)) {
        let ch_len = trimmed[last_break_pos..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        trimmed[last_break_pos + ch_len..].trim_matches(is_zero_width)
    } else {
        trimmed
    }
}

pub fn extract_word_immediate(before_cur: &str) -> &str {
    let before_cur = before_cur.trim_start_matches(is_zero_width);
    if let Some(last_break_pos) = before_cur.rfind(|c: char| is_word_break(c as u32)) {
        let ch_len = before_cur[last_break_pos..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        before_cur[last_break_pos + ch_len..].trim_matches(is_zero_width)
    } else {
        before_cur
    }
}

pub fn is_right_ident_separator(after_cursor: &str) -> bool {
    let mut chars = after_cursor.chars();
    match chars.next() {
        Some('_') | Some('/') => true,
        Some('.') => chars.next().map(|c| !c.is_whitespace()).unwrap_or(false),
        Some('-') => chars.next().map(|c| c.is_alphabetic()).unwrap_or(false),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn matches_last_composed_or_prefix(
    before_cur: &str,
    word: &str,
    last_composed: &str,
    prev_composed: &str,
    raw_buffer: &str,
    method: Method,
    last_replace_time: Option<std::time::Instant>,
    last_replace_cursor: Option<usize>,
    cursor: usize,
    max_surr_lag_ms: u64,
    max_surr_window_chars: usize,
) -> bool {
    let before_cur_clean = before_cur.trim_matches(is_zero_width);
    let before_cur_trimmed = before_cur_clean.trim_end_matches(' ');
    let mut matched = !last_composed.is_empty()
        && (before_cur_clean.ends_with(last_composed)
            || before_cur_trimmed.ends_with(last_composed)
            || (!prev_composed.is_empty()
                && (before_cur_clean.ends_with(prev_composed)
                    || before_cur_trimmed.ends_with(prev_composed)))
            || (!raw_buffer.is_empty()
                && (before_cur_clean.ends_with(raw_buffer)
                    || before_cur_trimmed.ends_with(raw_buffer))));

    if !matched && !last_composed.is_empty() && !word.is_empty() {
        for i in (1..last_composed.len()).rev() {
            if let Some(prefix) = last_composed.get(..i) {
                if word == prefix {
                    matched = true;
                    break;
                }
            }
        }
    }

    if !matched && !prev_composed.is_empty() && !word.is_empty() {
        for i in (1..prev_composed.len()).rev() {
            if let Some(prefix) = prev_composed.get(..i) {
                if word == prefix {
                    matched = true;
                    break;
                }
            }
        }
    }

    if !matched && !raw_buffer.is_empty() && !word.is_empty() {
        for i in (1..raw_buffer.len()).rev() {
            if let Some(prefix) = raw_buffer.get(..i) {
                if word == prefix {
                    matched = true;
                    break;
                }
            }
        }
    }

    if !matched && !raw_buffer.is_empty() && !word.is_empty() {
        let word_telex = decompose_to_raw(word, method);
        if raw_buffer.starts_with(&word_telex) || raw_buffer.starts_with(word) {
            matched = true;
        } else {
            let window_limit = std::cmp::max(
                max_surr_window_chars,
                raw_buffer.chars().count().saturating_add(2),
            );
            let is_stale_lag = match (last_replace_time, last_replace_cursor) {
                (Some(t), Some(c)) => {
                    t.elapsed().as_millis() <= max_surr_lag_ms as u128
                        && cursor >= c
                        && (cursor - c) <= window_limit
                }
                _ => false,
            };
            if is_stale_lag {
                let word_base = to_base_ascii(word);
                let composed_base = to_base_ascii(last_composed);
                if !word_base.is_empty()
                    && (composed_base == word_base
                        || composed_base.starts_with(&word_base)
                        || word_base.starts_with(&composed_base))
                {
                    matched = true;
                }
            }
        }
    }

    matched
}
