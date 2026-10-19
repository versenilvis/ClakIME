use crate::{engine, spelling, Method};
use std::collections::HashSet;
use std::sync::OnceLock;

static COMMON_ENGLISH_WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn common_english_words() -> &'static HashSet<&'static str> {
    COMMON_ENGLISH_WORDS.get_or_init(|| {
        [
            "a", "about", "after", "all", "also", "am", "an", "and", "any", "are", "as", "ask",
            "at", "back", "be", "because", "been", "best", "book", "boot", "but", "by", "can",
            "case", "cast", "check", "class", "close", "come", "const", "cook", "cool", "cost",
            "could", "data", "date", "day", "deep", "def", "desk", "did", "disk", "do", "does",
            "door", "east", "else", "end", "even", "fast", "feed", "file", "find", "first",
            "floor", "food", "fool", "foot", "for", "free", "from", "get", "give", "go", "good",
            "had", "has", "have", "he", "her", "hide", "him", "his", "home", "host", "how", "i",
            "if", "in", "info", "into", "is", "it", "its", "just", "keep", "know", "last", "let",
            "like", "line", "list", "log", "look", "loop", "main", "make", "mask", "me", "meet",
            "mood", "moon", "most", "must", "my", "name", "need", "new", "next", "no", "noon",
            "not", "now", "of", "on", "one", "only", "open", "or", "other", "our", "out", "over",
            "page", "part", "past", "path", "people", "pool", "poor", "port", "post", "read",
            "rest", "risk", "room", "root", "run", "say", "see", "seed", "set", "she", "show",
            "side", "size", "sleep", "so", "some", "soon", "sort", "speed", "start", "stop",
            "sweet", "take", "task", "test", "text", "than", "that", "the", "their", "them",
            "then", "there", "these", "they", "think", "this", "time", "to", "took", "tool",
            "tree", "true", "two", "type", "up", "us", "use", "var", "view", "want", "warn",
            "was", "way", "we", "week", "well", "west", "what", "when", "which", "while", "who",
            "wide", "will", "with", "wood", "word", "work", "would", "write", "year", "yes",
            "you", "your", "zoom",
        ]
        .into_iter()
        .collect()
    })
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
