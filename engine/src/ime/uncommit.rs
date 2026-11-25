pub fn try_match_uncommit(
    candidate: Option<(String, String, std::time::Instant)>,
    surrounding_text: Option<&str>,
    cursor: usize,
) -> Option<(String, String)> {
    let (cand_raw, cand_com, cand_time) = candidate?;
    if cand_time.elapsed().as_secs() > 10 {
        return None;
    }

    let mut matched = false;
    if let Some(text) = surrounding_text {
        let chars: Vec<char> = text.chars().collect();
        let cur = std::cmp::min(chars.len(), cursor);
        let before_cur: String = chars[..cur].iter().collect();
        let before_cur_trimmed = before_cur.trim_end_matches(' ');
        if cur > 0 && chars[cur - 1] == ' ' {
            let before_sp: String = chars[..cur - 1].iter().collect();
            if before_sp.ends_with(&cand_com) {
                matched = true;
            }
        }
        if !matched && (before_cur.ends_with(&cand_com) || before_cur_trimmed.ends_with(&cand_com))
        {
            matched = true;
        }
    } else {
        matched = true;
    }

    if matched {
        Some((cand_raw, cand_com))
    } else {
        None
    }
}
