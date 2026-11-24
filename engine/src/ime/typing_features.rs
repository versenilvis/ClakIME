use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SentenceCapState {
    StartOfInput,
    SawPunctuation,
    ReadyToCapitalize,
    Normal,
}

pub fn to_title_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut start_of_word = true;
    for c in s.chars() {
        if c.is_alphabetic() {
            if start_of_word {
                for up in c.to_uppercase() {
                    result.push(up);
                }
                start_of_word = false;
            } else {
                for low in c.to_lowercase() {
                    result.push(low);
                }
            }
        } else {
            start_of_word = true;
            result.push(c);
        }
    }
    result
}

pub fn match_macro(macros: &HashMap<String, String>, target: &str) -> Option<String> {
    if target.is_empty() {
        return None;
    }

    if let Some(repl) = macros.get(target) {
        return Some(repl.clone());
    }

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
                return Some(to_title_case(repl));
            } else {
                return Some(repl.clone());
            }
        }
    }

    None
}

pub fn is_word_break(ucs4: u32) -> bool {
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

pub enum BracketAction {
    UndoPair(&'static str, &'static str),
    Pair(&'static str),
}

pub fn check_bracket_shortcuts(
    enabled: bool,
    last_composed: &str,
    raw_buffer: &str,
    key_str: &str,
) -> Option<BracketAction> {
    if !enabled {
        return None;
    }

    if last_composed == "ơ" && key_str == "[" {
        Some(BracketAction::UndoPair("ơ", "["))
    } else if last_composed == "ư" && key_str == "]" {
        Some(BracketAction::UndoPair("ư", "]"))
    } else if last_composed == "Ơ" && key_str == "{" {
        Some(BracketAction::UndoPair("Ơ", "{"))
    } else if last_composed == "Ư" && key_str == "}" {
        Some(BracketAction::UndoPair("Ư", "}"))
    } else if raw_buffer.is_empty() {
        match key_str {
            "[" => Some(BracketAction::Pair("ơ")),
            "]" => Some(BracketAction::Pair("ư")),
            "{" => Some(BracketAction::Pair("Ơ")),
            "}" => Some(BracketAction::Pair("Ư")),
            _ => None,
        }
    } else {
        None
    }
}
