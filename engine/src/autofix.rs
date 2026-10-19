use crate::{engine, spelling, Method};
use std::collections::HashSet;

fn common_english_words() -> &'static HashSet<&'static str> {
    crate::english_dict::common_english_words()
}

// checks whether a word already contains vietnamese diacritics
fn has_vietnamese_accent(word: &str) -> bool {
    word.chars().any(|c| !c.is_ascii())
}

// checks whether a word is all uppercase (at least 2 letters, e.g. API, DDAY)
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

// checks if word is purely alphabetic letters
fn is_pure_letters(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_alphabetic())
}

/// Token in a document: Word, Whitespace, or Symbol
enum Token<'a> {
    Word(&'a str),
    NonWord(&'a str),
}

fn tokenize(input: &str, method: Method) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut in_word = false;

    let chars: Vec<(usize, char)> = input.char_indices().collect();
    for i in 0..chars.len() {
        let (byte_idx, ch) = chars[i];
        let is_word_ch = match method {
            Method::Vni | Method::TeipVni => ch.is_alphabetic() || ch.is_ascii_digit(),
            _ => ch.is_alphabetic(),
        };
        if i == 0 {
            in_word = is_word_ch;
            start = byte_idx;
        } else if is_word_ch != in_word {
            let slice = &input[start..byte_idx];
            if in_word {
                tokens.push(Token::Word(slice));
            } else {
                tokens.push(Token::NonWord(slice));
            }
            in_word = is_word_ch;
            start = byte_idx;
        }
    }

    if start < input.len() {
        let slice = &input[start..];
        if in_word {
            tokens.push(Token::Word(slice));
        } else {
            tokens.push(Token::NonWord(slice));
        }
    }

    tokens
}

/// Converts a selection of text conservatively:
/// - Skips words already having Vietnamese diacritics
/// - Skips ALL_CAPS words
/// - Skips common English words that collide with Telex
/// - Skips URLs, emails, identifiers with digits/symbols
/// - Transforms valid raw Telex/VNI words to Vietnamese if the result is a valid CVC syllable
/// Returns `(converted_string, was_changed)`
pub fn autofix_text(input: &str, method: Method, modern: bool) -> (String, bool) {
    if input.is_empty() {
        return (String::new(), false);
    }

    let tokens = tokenize(input, method);
    let mut output = String::with_capacity(input.len() + 16);
    let mut changed = false;

    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::NonWord(s) => {
                output.push_str(s);
            }
            Token::Word(w) => {
                // check if part of url, email, or code identifier (adjacent to _ or / or :// or @)
                let prev_non_word = if i > 0 {
                    if let Token::NonWord(p) = &tokens[i - 1] {
                        Some(*p)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let next_non_word = if i + 1 < tokens.len() {
                    if let Token::NonWord(n) = &tokens[i + 1] {
                        Some(*n)
                    } else {
                        None
                    }
                } else {
                    None
                };

                // URL / email / code identifier heuristics
                if let Some(prev) = prev_non_word {
                    if prev.ends_with("://")
                        || prev.ends_with('/')
                        || prev.ends_with('@')
                        || prev.ends_with('_')
                        || prev.ends_with('.')
                    {
                        output.push_str(w);
                        continue;
                    }
                }

                if let Some(next) = next_non_word {
                    if next.starts_with('@')
                        || next.starts_with('_')
                        || next.starts_with('/')
                        || next.starts_with("://")
                        || (next.starts_with('.')
                            && next.chars().nth(1).map_or(false, |c| c.is_alphabetic()))
                    {
                        output.push_str(w);
                        continue;
                    }
                }

                // 1. already has vietnamese accents -> keep as is
                if has_vietnamese_accent(w) {
                    output.push_str(w);
                    continue;
                }

                // 2. all uppercase (acronyms, constants) -> keep as is
                if is_all_uppercase(w) {
                    output.push_str(w);
                    continue;
                }

                // 3. must be valid letters or VNI candidate
                let is_valid_candidate = match method {
                    Method::Vni | Method::TeipVni => {
                        w.chars().any(|c| c.is_alphabetic())
                            && w.chars().next().map_or(false, |c| c.is_alphabetic())
                    }
                    _ => is_pure_letters(w),
                };
                if !is_valid_candidate {
                    output.push_str(w);
                    continue;
                }

                // 4. common english words -> keep as is
                let lower_w = w.to_lowercase();
                if common_english_words().contains(lower_w.as_str()) {
                    output.push_str(w);
                    continue;
                }

                // 5. transform through engine
                let converted = match method {
                    Method::Telex => engine::convert_telex(w, modern, false),
                    Method::Vni => engine::convert_vni(w, modern, false),
                    Method::TeipVni => engine::convert_teip_vni(w, modern, false),
                    Method::Viqr => engine::convert_viqr(w),
                };

                // 6. verify result
                if converted != *w && spelling::is_valid_cvc(&converted) {
                    output.push_str(&converted);
                    changed = true;
                } else {
                    output.push_str(w);
                }
            }
        }
    }

    (output, changed)
}
