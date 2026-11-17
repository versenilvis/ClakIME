// shortcut formatters and parsers for clak settings ui

pub fn shortcut_id_to_display(id: &str) -> String {
    let lower = id.trim().to_lowercase();
    match lower.as_str() {
        "ctrl_shift" | "ctrl+shift" => "Ctrl + Shift".to_string(),
        "alt_z" | "alt+z" => "Alt + Z".to_string(),
        "super_space" | "super+space" => "Super + Space".to_string(),
        "ctrl_space" | "ctrl+space" => "Ctrl + Space".to_string(),
        "shift_space" | "shift+space" => "Shift + Space".to_string(),
        "alt_shift" | "alt+shift" => "Alt + Shift".to_string(),
        "alt_space" | "alt+space" => "Alt + Space".to_string(),
        other => {
            if other.is_empty() {
                "Ctrl + Shift".to_string()
            } else {
                other
                    .replace('_', " + ")
                    .split(" + ")
                    .map(|p| {
                        let mut chars = p.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" + ")
            }
        }
    }
}

pub fn shortcut_display_to_id(display: &str) -> String {
    let s = display
        .trim()
        .to_lowercase()
        .replace(" + ", "_")
        .replace(' ', "_");
    if s.is_empty() {
        "ctrl_shift".to_string()
    } else {
        s
    }
}

pub fn format_recorded_shortcut(
    key_text: &str,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
) -> Option<(String, String)> {
    let is_space = key_text == " " || key_text.eq_ignore_ascii_case("space");

    // Case 1: Super / Meta (Windows key) combinations (e.g. Super + Space)
    if meta {
        if is_space {
            return Some(("Super + Space".to_string(), "super_space".to_string()));
        }
        let clean = key_text.trim();
        if !clean.is_empty() && clean.chars().any(|c| c.is_alphanumeric()) {
            let u = clean.to_uppercase();
            let l = clean.to_lowercase();
            return Some((format!("Super + {}", u), format!("super_{}", l)));
        }
        return None;
    }

    // Case 2: Ctrl + Shift
    if ctrl && shift {
        if is_space {
            return Some((
                "Ctrl + Shift + Space".to_string(),
                "ctrl_shift_space".to_string(),
            ));
        }
        return Some(("Ctrl + Shift".to_string(), "ctrl_shift".to_string()));
    }

    // Case 3: Alt + Shift
    if alt && shift {
        return Some(("Alt + Shift".to_string(), "alt_shift".to_string()));
    }

    // Case 4: Ctrl + key (e.g. Ctrl + Space)
    if ctrl && !alt {
        if is_space {
            return Some(("Ctrl + Space".to_string(), "ctrl_space".to_string()));
        }
        let clean = key_text.trim();
        if !clean.is_empty()
            && clean
                .chars()
                .any(|c| c.is_alphanumeric() || c.is_ascii_punctuation())
        {
            let u = clean.to_uppercase();
            let l = clean.to_lowercase();
            return Some((format!("Ctrl + {}", u), format!("ctrl_{}", l)));
        }
        return None;
    }

    // Case 5: Alt + key (e.g. Alt + Z, Alt + Space)
    if alt && !ctrl {
        if is_space {
            return Some(("Alt + Space".to_string(), "alt_space".to_string()));
        }
        let clean = key_text.trim();
        if !clean.is_empty()
            && clean
                .chars()
                .any(|c| c.is_alphanumeric() || c.is_ascii_punctuation())
        {
            let u = clean.to_uppercase();
            let l = clean.to_lowercase();
            return Some((format!("Alt + {}", u), format!("alt_{}", l)));
        }
        return None;
    }

    // Case 6: Shift + Space
    if shift && !ctrl && !alt {
        if is_space {
            return Some(("Shift + Space".to_string(), "shift_space".to_string()));
        }
        return None;
    }

    None
}
