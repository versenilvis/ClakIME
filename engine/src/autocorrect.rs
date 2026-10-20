use crate::{engine, spelling};

pub use crate::english_dict::is_english;

// checks if word is all uppercase acronym/constant (e.g. API, URL, HTML, DDAY)
fn is_all_uppercase(word: &str) -> bool {
    let mut letter_count = 0;
    for c in word.chars() {
        if c.is_alphabetic() {
            letter_count += 1;
            if !c.is_uppercase() {
                return false;
            }
        }
    }
    letter_count >= 2
}

// checks if word has mixed camelCase or PascalCase (e.g. iPhone, CamelCase)
fn is_mixed_case(word: &str) -> bool {
    let mut saw_lower = false;
    let mut saw_upper_after_lower = false;
    for (i, c) in word.chars().enumerate() {
        if c.is_lowercase() {
            saw_lower = true;
        } else if c.is_uppercase() && i > 0 && saw_lower {
            saw_upper_after_lower = true;
            break;
        }
    }
    saw_upper_after_lower
}

// checks whether a word already contains vietnamese diacritics
pub fn has_vietnamese_accent(word: &str) -> bool {
    !word.is_ascii()
}

// checks if raw string has telex-specific modifier or tone characters
fn has_telex_markers(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    lower.contains('w')
        || lower.contains('f')
        || lower.contains('s')
        || lower.contains('r')
        || lower.contains('x')
        || lower.contains('j')
        || lower.matches('a').count() >= 2
        || lower.matches('e').count() >= 2
        || lower.matches('o').count() >= 2
        || lower.matches('d').count() >= 2
}

// match casing of original input word (uppercase, capitalized, or lowercase)
fn match_casing(original: &str, target: &str) -> String {
    if original.is_empty() || target.is_empty() {
        return target.to_string();
    }
    let first_char = original.chars().next().unwrap();
    if first_char.is_uppercase() {
        let mut chars = target.chars();
        if let Some(c) = chars.next() {
            let mut res = c.to_uppercase().to_string();
            res.extend(chars);
            return res;
        }
    }
    target.to_string()
}

// calculates minimum and maximum expected telex keystrokes to type word
fn telex_keystroke_bounds(word: &str) -> (usize, usize) {
    let mut min_keys = 0;
    let mut max_keys = 0;
    let mut has_u_horn = false;
    let mut has_o_horn = false;

    for c in word.chars() {
        match c {
            'đ' | 'Đ' => {
                min_keys += 2;
                max_keys += 2;
            }
            'â' | 'Â' | 'ê' | 'Ê' | 'ô' | 'Ô' => {
                min_keys += 2;
                max_keys += 2;
            }
            'ấ' | 'Ấ' | 'ầ' | 'Ầ' | 'ẩ' | 'Ẩ' | 'ẫ' | 'Ẫ' | 'ậ' | 'Ậ' | 'ế' | 'Ế' | 'ề' | 'Ề'
            | 'ể' | 'Ể' | 'ễ' | 'Ễ' | 'ệ' | 'Ệ' | 'ố' | 'Ố' | 'ồ' | 'Ồ' | 'ổ' | 'Ổ' | 'ỗ' | 'Ỗ'
            | 'ộ' | 'Ộ' => {
                min_keys += 3;
                max_keys += 3;
            }
            'ă' | 'Ă' => {
                min_keys += 2;
                max_keys += 2;
            }
            'ắ' | 'Ắ' | 'ằ' | 'Ằ' | 'ẳ' | 'Ẳ' | 'ẵ' | 'Ẵ' | 'ặ' | 'Ặ' => {
                min_keys += 3;
                max_keys += 3;
            }
            'ư' | 'Ư' => {
                has_u_horn = true;
                min_keys += 2;
                max_keys += 2;
            }
            'ứ' | 'Ứ' | 'ừ' | 'Ừ' | 'ử' | 'Ử' | 'ữ' | 'Ữ' | 'ự' | 'Ự' => {
                has_u_horn = true;
                min_keys += 3;
                max_keys += 3;
            }
            'ơ' | 'Ơ' => {
                has_o_horn = true;
                min_keys += 2;
                max_keys += 2;
            }
            'ớ' | 'Ớ' | 'ờ' | 'Ờ' | 'ở' | 'Ở' | 'ỡ' | 'Ỡ' | 'ợ' | 'Ợ' => {
                has_o_horn = true;
                min_keys += 3;
                max_keys += 3;
            }
            'á' | 'Á' | 'à' | 'À' | 'ả' | 'Ả' | 'ã' | 'Ã' | 'ạ' | 'Ạ' | 'é' | 'É' | 'è' | 'È'
            | 'ẻ' | 'Ẻ' | 'ẽ' | 'Ẽ' | 'ẹ' | 'Ẹ' | 'í' | 'Í' | 'ì' | 'Ì' | 'ỉ' | 'Ỉ' | 'ĩ' | 'Ĩ'
            | 'ị' | 'Ị' | 'ó' | 'Ó' | 'ò' | 'Ò' | 'ỏ' | 'Ỏ' | 'õ' | 'Õ' | 'ọ' | 'Ọ' | 'ú' | 'Ú'
            | 'ù' | 'Ù' | 'ủ' | 'Ủ' | 'ũ' | 'Ũ' | 'ụ' | 'Ụ' | 'ý' | 'Ý' | 'ỳ' | 'Ỳ' | 'ỷ' | 'Ỷ'
            | 'ỹ' | 'Ỹ' | 'ỵ' | 'Ỵ' => {
                min_keys += 2;
                max_keys += 2;
            }
            _ => {
                min_keys += 1;
                max_keys += 1;
            }
        }
    }

    if has_u_horn && has_o_horn && min_keys > 1 {
        min_keys -= 1;
    }

    (min_keys, max_keys)
}

// checks if candidate word matches raw initial consonant
fn matches_initial_consonant(raw_lower: &str, ic_len: usize, candidate: &str) -> bool {
    let cand_lower = candidate.to_lowercase();
    if cand_lower.starts_with('đ') {
        return raw_lower.starts_with('d') && raw_lower.matches('d').count() >= 2;
    }
    if ic_len == 0 {
        // raw started with vowel, candidate must start with same first vowel letter
        let raw_first = raw_lower.chars().next();
        let cand_first = cand_lower.chars().next();
        match (raw_first, cand_first) {
            (Some(r), Some(c)) => {
                let norm_c = match c {
                    'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'â' | 'ấ' | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' | 'ă' | 'ắ'
                    | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' => 'a',
                    'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => {
                        'e'
                    }
                    'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
                    'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ'
                    | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
                    'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => {
                        'u'
                    }
                    'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
                    other => other,
                };
                r == norm_c
            }
            _ => false,
        }
    } else {
        cand_lower.starts_with(&raw_lower[..ic_len])
    }
}

// validates candidate word against cvc rules and vietnamese dictionary
fn is_valid_vietnamese_word(candidate: &str) -> bool {
    if !spelling::is_valid_cvc(candidate) {
        return false;
    }
    let lower = candidate.to_lowercase();
    crate::dict().binary_search(&lower.as_str()).is_ok()
}

/// Attempts to autocorrect a scrambled or mis-typed Telex word into a valid Vietnamese word.
/// Returns `Some(corrected_word)` if and only if a SINGLE unambiguous match is found.
pub fn try_autocorrect_telex(raw: &str, modern: bool, short_w: bool) -> Option<String> {
    if raw.len() < 3 || raw.len() > 10 {
        return None;
    }

    // safety guard 1: skip non-alphabetic, numbers, symbols
    if !raw.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    // safety guard 2: skip all-uppercase (acronyms) and camelCase
    if is_all_uppercase(raw) || is_mixed_case(raw) {
        return None;
    }

    let lower = raw.to_lowercase();

    // safety guard 3: skip known english/tech words
    if is_english(&lower) {
        return None;
    }

    // check learned typos first (adaptive learning)
    if let Some(target) = crate::learned_typos::lookup_learned_typo(&lower) {
        return Some(match_casing(raw, &target));
    }

    // famous omitted-circumflex typo
    if lower == "khnog" {
        return Some(match_casing(raw, "không"));
    }

    // safety guard 4: must contain unambiguous telex markers
    if !has_telex_markers(&lower) {
        return None;
    }

    // safety guard 5: consecutive tone modifiers indicate user untoned on purpose (e.g. ass -> as, iss -> is)
    if lower.contains("ss")
        || lower.contains("ff")
        || lower.contains("rr")
        || lower.contains("xx")
        || lower.contains("jj")
    {
        return None;
    }

    // safety guard 6: initial consonant must strictly adhere to vietnamese orthography
    let ic_len = if lower.starts_with("ngh") {
        3
    } else if lower.starts_with("ch")
        || lower.starts_with("gh")
        || lower.starts_with("gi")
        || lower.starts_with("kh")
        || lower.starts_with("nh")
        || lower.starts_with("ng")
        || lower.starts_with("ph")
        || lower.starts_with("qu")
        || lower.starts_with("th")
        || lower.starts_with("tr")
        || lower.starts_with("dd")
    {
        2
    } else if lower.starts_with(|c: char| {
        matches!(
            c,
            'b' | 'c' | 'd' | 'g' | 'h' | 'k' | 'l' | 'm' | 'n' | 'p' | 'r' | 's' | 't' | 'v' | 'x'
        )
    }) {
        1
    } else if lower.starts_with(['a', 'e', 'i', 'o', 'u', 'y']) {
        0
    } else {
        // words starting with f, j, w, z or illegal consonants are never vietnamese
        return None;
    };

    // foreign consonant clusters never occur in vietnamese
    if lower.starts_with("thr")
        || lower.starts_with("shr")
        || lower.starts_with("str")
        || lower.starts_with("spr")
        || lower.starts_with("scr")
        || lower.starts_with("spl")
        || lower.starts_with("br")
        || lower.starts_with("cr")
        || lower.starts_with("dr")
        || lower.starts_with("gr")
        || lower.starts_with("pr")
        || lower.starts_with("fr")
        || lower.starts_with("bl")
        || lower.starts_with("cl")
        || lower.starts_with("gl")
        || lower.starts_with("pl")
        || lower.starts_with("sl")
        || lower.starts_with("fl")
        || lower.starts_with("wr")
        || lower.starts_with("kn")
        || lower.starts_with("ps")
        || lower.starts_with("phr")
    {
        return None;
    }

    let rest = &lower[ic_len..];
    if rest.is_empty() {
        return None;
    }

    // tone marker or non-coda consonant cannot precede vowels in vietnamese syllable
    if rest.starts_with(['s', 'f', 'r', 'x', 'j', 'l']) {
        return None;
    }

    // rest must contain at least one vowel
    if !rest
        .chars()
        .any(|c| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'w'))
    {
        return None;
    }

    // tone key budget: in telex, rest can have at most one tone marker
    let tone_count = rest
        .chars()
        .filter(|&c| matches!(c, 's' | 'f' | 'r' | 'x' | 'j'))
        .count();
    if tone_count > 1 {
        return None;
    }

    // if raw itself already converts to an english word, skip
    let direct_conv = engine::convert_telex(&lower, modern, short_w);
    if is_english(&direct_conv) || is_english(&lower) {
        return None;
    }

    let is_duoc_scramble = (direct_conv == "được" || direct_conv == "Được")
        && (lower.contains("dud") || lower.contains("wjc") || lower.contains("wocj"));

    // if raw itself already converts to a valid dictionary word, no autocorrect needed
    // unless it is a known scramble of duoc
    if !is_duoc_scramble && is_valid_vietnamese_word(&direct_conv) {
        return None;
    }

    let chars: Vec<char> = lower.chars().collect();
    let n = chars.len();
    // initial consonant is immutable; swaps only occur at index >= start_idx
    let start_idx = ic_len;
    if start_idx >= n - 1 {
        return None;
    }

    let mut candidates: Vec<String> = Vec::new();

    // if direct conversion is duoc scramble, prioritize it
    if is_duoc_scramble && is_valid_vietnamese_word(&direct_conv) {
        candidates.push(direct_conv);
    }

    let test_permutation = |trial_chars: &[char], candidates: &mut Vec<String>| {
        let trial_raw: String = trial_chars.iter().collect();
        let conv = engine::convert_telex(&trial_raw, modern, short_w);
        if !is_valid_vietnamese_word(&conv) {
            return;
        }
        if is_english(&conv) {
            return;
        }
        if !matches_initial_consonant(&lower, ic_len, &conv) {
            return;
        }
        // keystroke conservation check: ensure no keys were swallowed
        let (min_k, max_k) = telex_keystroke_bounds(&conv);
        if n < min_k || n > max_k {
            return;
        }
        if !candidates.contains(&conv) {
            candidates.push(conv);
        }
    };

    // level 1: single adjacent transposition (Kendall-tau distance = 1)
    // covers thoogn -> thoong, nguwoif (if swapped), etc.
    for i in start_idx..n - 1 {
        let mut trial = chars.clone();
        trial.swap(i, i + 1);
        test_permutation(&trial, &mut candidates);
    }

    // level 2: exactly two adjacent transpositions (Kendall-tau distance = 2)
    // only checked if level 1 found no candidates (bounded search space)
    if candidates.is_empty() {
        // case a: two disjoint adjacent swaps (e.g. nheiuef -> nhieeuf)
        if n >= start_idx + 4 {
            for i in start_idx..n - 3 {
                for j in (i + 2)..n - 1 {
                    let mut trial = chars.clone();
                    trial.swap(i, i + 1);
                    trial.swap(j, j + 1);
                    test_permutation(&trial, &mut candidates);
                }
            }
        }

        // case b: 3-element shifts (consecutive overlapping swaps, e.g. beeits -> bieets)
        if n >= start_idx + 3 {
            for i in start_idx..n - 2 {
                // shift left: move element at i+2 to i (beeits -> bieets)
                let mut trial_left = chars.clone();
                trial_left.swap(i + 1, i + 2);
                trial_left.swap(i, i + 1);
                test_permutation(&trial_left, &mut candidates);

                // shift right: move element at i to i+2
                let mut trial_right = chars.clone();
                trial_right.swap(i, i + 1);
                trial_right.swap(i + 1, i + 2);
                test_permutation(&trial_right, &mut candidates);
            }
        }
    }

    // level 3: vietnamese triphthong circumflex expansion (ieu -> ieeu, yeu -> yeeu)
    // in vietnamese orthography, plain 'e' never exists in 'ieu' or 'yeu'
    if candidates.is_empty() && (lower.contains("ieu") || lower.contains("yeu")) {
        let expanded = if lower.contains("ieu") {
            lower.replace("ieu", "ieeu")
        } else {
            lower.replace("yeu", "yeeu")
        };
        let conv = engine::convert_telex(&expanded, modern, short_w);
        if is_valid_vietnamese_word(&conv)
            && !is_english(&conv)
            && matches_initial_consonant(&lower, ic_len, &conv)
        {
            let (min_k, max_k) = telex_keystroke_bounds(&conv);
            if n + 1 >= min_k && n < max_k && !candidates.contains(&conv) {
                candidates.push(conv);
            }
        }
    }

    // strict requirement: only autocorrect if there is EXACTLY ONE candidate
    // if there are 2 or more candidates, the word is ambiguous -> do not change
    if candidates.len() == 1 {
        Some(match_casing(raw, &candidates[0]))
    } else {
        None
    }
}
