use crate::tables;

#[derive(Clone, Copy, Default)]
pub(crate) struct Letter {
    pub(crate) c: char,
    pub(crate) is_vowel: bool,
    pub(crate) variant: u8,
    pub(crate) tone: u8,
    pub(crate) is_dstroke: bool,
    pub(crate) upper: bool,
    pub(crate) from_w: bool,
    pub(crate) horn_propagated: bool,
    pub(crate) circ_toggled: bool,
    pub(crate) d_toggled: bool,
    pub(crate) horn_toggled: bool,
}

impl Letter {
    pub(crate) fn new(c: char, upper: bool) -> Self {
        let lc = c.to_ascii_lowercase();
        Letter {
            c: lc,
            is_vowel: tables::is_vowel(lc),
            variant: 0,
            tone: 0,
            is_dstroke: false,
            upper,
            from_w: false,
            horn_propagated: false,
            circ_toggled: false,
            d_toggled: false,
            horn_toggled: false,
        }
    }
}

fn emit(ls: &[Letter]) -> String {
    let mut out = String::with_capacity(ls.len() * 3);
    for lt in ls {
        if lt.is_dstroke {
            out.push_str(if lt.upper {
                tables::D_STROKE_UP
            } else {
                tables::D_STROKE_LO
            });
        } else if lt.is_vowel {
            out.push_str(tables::vowel_utf8(lt.c, lt.variant, lt.tone, lt.upper));
        } else {
            out.push(if lt.upper {
                lt.c.to_ascii_uppercase()
            } else {
                lt.c
            });
        }
    }
    out
}

/// Validity gate for a NON-ADJACENT roof application (trailing chars sit
/// between the new key and the target vowel).  The delayed-roof feature
/// intentionally crosses valid final consonants ("nhan"+a → "nhân"), but
/// the same loop also crossed consonants that can never close a syllable:
/// "ava" → "âv", and the aaa-unmerge round-trip turned "avata" into
/// "avta".  Only fire the merge/swap when the candidate word is a valid
/// Vietnamese syllable.  `swap` distinguishes the hook↔circumflex swap
/// (variant 2→1, which also unhooks a preceding hooked u for o) from the
/// plain merge (variant 0→1).  Adjacent applications skip the gate.
fn crossing_roof_valid(ls: &[Letter], i: usize, upper: bool, swap: bool) -> bool {
    if i + 1 >= ls.len() {
        return true;
    }
    let mut trial = ls.to_vec();
    trial[i].variant = 1;
    if upper {
        trial[i].upper = true;
    }
    if swap
        && trial[i].c == 'o'
        && i > 0
        && trial[i - 1].is_vowel
        && trial[i - 1].c == 'u'
        && trial[i - 1].variant == 2
    {
        trial[i - 1].variant = 0;
    }
    crate::spelling::is_valid_cvc(&emit(&trial))
}

fn last_vowel_index(ls: &[Letter]) -> Option<usize> {
    ls.iter().rposition(|lt| lt.is_vowel)
}
fn last_d_index(ls: &[Letter]) -> Option<usize> {
    ls.iter()
        .rposition(|lt| lt.c == 'd' && !lt.is_vowel && !lt.is_dstroke)
}
fn last_dstroke_index(ls: &[Letter]) -> Option<usize> {
    ls.iter().rposition(|lt| lt.is_dstroke)
}

fn is_valid_vietnamese_initial(initial: &[Letter], first_vowel: char) -> bool {
    let fv = first_vowel.to_ascii_lowercase();
    match initial.len() {
        0 => true,
        1 => {
            let c0 = initial[0].c.to_ascii_lowercase();
            match c0 {
                'j' | 'w' | 'z' => false,
                'k' => matches!(fv, 'e' | 'i' | 'y'),
                'c' => !matches!(fv, 'e' | 'i' | 'y'),
                'q' => fv == 'u',
                'g' => fv != 'e',
                _ => true,
            }
        }
        2 => {
            let c0 = initial[0].c.to_ascii_lowercase();
            let c1 = initial[1].c.to_ascii_lowercase();
            match (c0, c1) {
                ('g', 'h') => matches!(fv, 'e' | 'i'),
                ('n', 'g') => !matches!(fv, 'e' | 'i'),
                ('c', 'h')
                | ('g', 'i')
                | ('k', 'h')
                | ('n', 'h')
                | ('p', 'h')
                | ('q', 'u')
                | ('t', 'h')
                | ('t', 'r') => true,
                _ => false,
            }
        }
        3 => {
            let c0 = initial[0].c.to_ascii_lowercase();
            let c1 = initial[1].c.to_ascii_lowercase();
            let c2 = initial[2].c.to_ascii_lowercase();
            c0 == 'n' && c1 == 'g' && c2 == 'h' && matches!(fv, 'e' | 'i')
        }
        _ => false,
    }
}

fn is_valid_vietnamese_coda(coda: &[Letter]) -> bool {
    match coda.len() {
        0 => true,
        1 => matches!(
            coda[0].c.to_ascii_lowercase(),
            'c' | 'm' | 'n' | 'p' | 't' | 'w'
        ),
        2 => {
            let c0 = coda[0].c.to_ascii_lowercase();
            let c1 = coda[1].c.to_ascii_lowercase();
            matches!((c0, c1), ('c', 'h') | ('n', 'g') | ('n', 'h'))
        }
        _ => false,
    }
}

fn tone_vowel_index(ls: &[Letter], modern: bool) -> Option<usize> {
    let end = last_vowel_index(ls)?;
    let mut start = end;
    while start > 0 && ls[start - 1].is_vowel {
        start -= 1;
    }
    if start < end
        && start > 0
        && ((ls[start].c == 'u' && ls[start - 1].c == 'q')
            || (ls[start].c == 'i' && ls[start - 1].c == 'g'))
    {
        start += 1;
    }
    if start > end {
        start = end;
    }
    // disallow tone on invalid initial consonants or coda
    if start > 0 && !is_valid_vietnamese_initial(&ls[..start], ls[start].c) {
        return None;
    }
    if end + 1 < ls.len() && !is_valid_vietnamese_coda(&ls[end + 1..]) {
        return None;
    }
    let count = end - start + 1;
    if count < 3 {
        for i in (start..=end).rev() {
            if ls[i].variant != 0 {
                return Some(i);
            }
        }
    }
    if count == 1 {
        return Some(start);
    }
    if count >= 3 {
        if ls[start].c == 'u'
            && start + 2 <= end
            && ls[start + 1].c == 'y'
            && ls[start + 2].c == 'e'
        {
            return Some(start + 2);
        }
        return Some(start + 1);
    }
    if end + 1 < ls.len() {
        return Some(end);
    }
    let (c1, c2) = (ls[start].c, ls[end].c);
    if (c1 == 'o' && (c2 == 'a' || c2 == 'e')) || (c1 == 'u' && c2 == 'y') {
        return if modern { Some(end) } else { Some(start) };
    }
    Some(start)
}

/// After a hook change (w key), recalculate the correct tone position and
/// move the tone if it's on the wrong vowel. Ported from fcitx5-unikey's
/// tone re-positioning in processHook / processHookWithUO.
fn reposition_tone_after_hook(ls: &mut [Letter], modern: bool) {
    let end = match last_vowel_index(ls) {
        Some(e) => e,
        None => return,
    };
    let mut start = end;
    while start > 0 && ls[start - 1].is_vowel {
        start -= 1;
    }
    // Skip gi/qu digraph vowels
    if start < end
        && start > 0
        && ((ls[start].c == 'u' && ls[start - 1].c == 'q')
            || (ls[start].c == 'i' && ls[start - 1].c == 'g'))
    {
        start += 1;
    }
    if start > end {
        return;
    }

    // Find which vowel in the cluster currently has tone
    let cur_tone_idx = (start..=end).find(|&i| ls[i].tone != 0);
    let tone_val = match cur_tone_idx {
        Some(i) => ls[i].tone,
        None => return, // no tone → nothing to reposition
    };

    // Find where tone SHOULD be after the hook change
    let new_tone_idx = match tone_vowel_index(ls, modern) {
        Some(i) => i,
        None => return,
    };

    // Move tone if position changed
    if Some(new_tone_idx) != cur_tone_idx {
        if let Some(old) = cur_tone_idx {
            ls[old].tone = 0;
        }
        ls[new_tone_idx].tone = tone_val;
    }
}

fn telex_tone(c: char) -> Option<u8> {
    match c.to_ascii_lowercase() {
        's' => Some(2),
        'f' => Some(1),
        'r' => Some(3),
        'x' => Some(4),
        'j' => Some(5),
        _ => None,
    }
}
fn vni_tone(c: char) -> Option<u8> {
    match c {
        '1' => Some(2),
        '2' => Some(1),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        _ => None,
    }
}

pub fn convert_telex(input: &str, modern: bool, short_w: bool) -> String {
    convert_telex_impl(input, modern, short_w)
}

/// Telex conversion with auto-restore: if the composed output is not valid
/// Vietnamese and differs from the raw input, return the raw input instead.
/// Skips auto-restore when the input has Vietnamese composition markers
/// (dd, vowel+w, double vowels): those signal intentional IME transforms
pub fn convert_telex_auto_restore(input: &str, modern: bool, short_w: bool) -> String {
    let composed = convert_telex_impl(input, modern, short_w);
    let all_ascii = composed.is_ascii();
    let swallowed_char = !crate::has_consecutive_modifiers(input) && input.len() > composed.len();
    if composed != input
        && (!all_ascii || swallowed_char)
        && !crate::spelling::is_valid_cvc(&composed)
        && !crate::has_vn_markers(input)
    {
        input.to_string()
    } else {
        composed
    }
}

fn convert_telex_impl(input: &str, modern: bool, short_w: bool) -> String {
    let mut ls: Vec<Letter> = Vec::with_capacity(input.len());
    let mut reverted = false;
    for ch in input.chars() {
        let lc = ch.to_ascii_lowercase();
        let upper = ch.is_ascii_uppercase();
        if reverted {
            ls.push(Letter::new(ch, upper));
            continue;
        }
        if lc == 'd' {
            let any_d_toggled = ls.iter().any(|lt| lt.c == 'd' && lt.d_toggled);
            if !any_d_toggled && last_dstroke_index(&ls).is_none() {
                if let Some(di) = last_d_index(&ls) {
                    ls[di].is_dstroke = true;
                    if upper {
                        ls[di].upper = true;
                    }
                    continue;
                }
            } else if last_dstroke_index(&ls).is_some() {
                if let Some(di) = last_dstroke_index(&ls) {
                    ls[di].is_dstroke = false;
                    ls[di].d_toggled = true;
                    reverted = true;
                }
            }
        }
        if matches!(lc, 'a' | 'e' | 'o') {
            let mut consumed = false;
            if let Some(vi) = last_vowel_index(&ls) {
                // If any matching vowel was already toggled off, don't reapply.
                let any_circ_toggled = ls[..=vi]
                    .iter()
                    .any(|lt| lt.is_vowel && lt.c == lc && lt.circ_toggled);
                if !any_circ_toggled {
                    for i in (0..=vi).rev() {
                        if ls[i].is_vowel {
                            if ls[i].c == lc {
                                if i < vi {
                                    // Roof crossing rules (x-unikey VSeqList
                                    // withRoof): a crosses y/u (ây, âu),
                                    // e crosses u (êu), o crosses i
                                    // (ôi, uôi). Any other intermediate
                                    // vowel blocks the roof.
                                    let blocked = ls[i + 1..=vi].iter().any(|lt| {
                                        lt.is_vowel
                                            && !matches!(
                                                (lc, lt.c),
                                                ('a', 'y' | 'u') | ('e', 'u') | ('o', 'i')
                                            )
                                    });
                                    if blocked {
                                        break;
                                    }
                                }
                                if ls[i].variant == 0 {
                                    if !crossing_roof_valid(&ls, i, upper, false) {
                                        break;
                                    }
                                    ls[i].variant = 1;
                                    if upper {
                                        ls[i].upper = true;
                                    }
                                    consumed = true;
                                    break;
                                }
                                if ls[i].variant == 1 {
                                    if i + 1 < ls.len() {
                                        let mut trial = ls.to_vec();
                                        trial[i].variant = 0;
                                        if !crate::spelling::is_valid_cvc(&emit(&trial)) {
                                            break;
                                        }
                                    }
                                    ls[i].variant = 0;
                                    ls[i].circ_toggled = true;
                                    reverted = true;
                                    break;
                                } // unmerge once
                                if ls[i].variant == 2 {
                                    if !crossing_roof_valid(&ls, i, upper, true) {
                                        break;
                                    }
                                    ls[i].variant = 1;
                                    if upper {
                                        ls[i].upper = true;
                                    }
                                    if ls[i].c == 'o'
                                        && i > 0
                                        && ls[i - 1].is_vowel
                                        && ls[i - 1].c == 'u'
                                        && ls[i - 1].variant == 2
                                    {
                                        ls[i - 1].variant = 0;
                                    }
                                    consumed = true;
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
            if consumed {
                continue;
            }
        }
        if lc == 'w' {
            let hook_result = crate::vseq::try_apply_hook(&mut ls);
            match hook_result {
                crate::vseq::HookResult::Applied => {
                    reposition_tone_after_hook(&mut ls, modern);
                    continue;
                }
                crate::vseq::HookResult::ToggledOff => {
                    reposition_tone_after_hook(&mut ls, modern);
                    reverted = true;
                }
                crate::vseq::HookResult::ToggledToLiteral => {
                    reverted = true;
                    continue;
                }
                crate::vseq::HookResult::NotApplicable => {
                    if crate::vseq::try_insert_horn(&mut ls, short_w, upper) {
                        continue;
                    }
                }
            }
        }
        if let Some(t) = telex_tone(lc) {
            if let Some(vi) = tone_vowel_index(&ls, modern) {
                if ls[vi].tone == t {
                    ls[vi].tone = 0;
                    reverted = true;
                } else {
                    ls[vi].tone = t;
                    continue;
                }
            }
        }
        if lc == 'z' {
            if let Some(vi) = tone_vowel_index(&ls, modern) {
                if ls[vi].tone != 0 {
                    ls[vi].tone = 0;
                    continue;
                }
            }
        }
        ls.push(Letter::new(ch, upper));
        // Horn propagation: ư+o→ươ, ơ+u→ươ within vowel cluster.
        // When a horned vowel is followed by a compatible vowel, share horn.
        if ls.len() >= 2 {
            let last = ls.len() - 1;
            if ls[last].is_vowel && ls[last].variant == 0 {
                let prev = last - 1;
                if ls[prev].is_vowel && ls[prev].variant == 2 {
                    if ls[prev].c == 'u' && ls[last].c == 'o' {
                        ls[last].variant = 2;
                        ls[last].horn_propagated = true;
                    }
                    if ls[prev].c == 'o' && ls[last].c == 'u' {
                        let has_u_horn_before = prev >= 1
                            && ls[prev - 1].is_vowel
                            && ls[prev - 1].c == 'u'
                            && ls[prev - 1].variant == 2;
                        if !has_u_horn_before {
                            ls[last].variant = 2;
                            ls[last].horn_propagated = true;
                        }
                    }
                }
            }
        }
        // Tone reassignment: when a new vowel is pushed, move tone if the
        // vowel structure has changed.
        // e.g. "gis"→"gí" then "a"→"giá" (tone moves from i to a).
        // e.g. "ngoaf"→"ngòa" then "i"→"ngoài" (triphthong, tone to middle).
        if ls.len() >= 2 {
            let last = ls.len() - 1;
            if ls[last].is_vowel {
                // Find first vowel in the cluster
                let first_vowel = (0..last).find(|&j| ls[j].is_vowel);
                if let Some(fv) = first_vowel {
                    let vcount = last - fv + 1;
                    // gi/qu digraph: move tone from digraph vowel to new vowel
                    if vcount >= 2 {
                        for j in (fv..last).rev() {
                            if ls[j].tone != 0 {
                                let move_tone = (ls[j].c == 'i' && j > 0 && ls[j - 1].c == 'g')
                                    || (ls[j].c == 'u' && j > 0 && ls[j - 1].c == 'q');
                                if move_tone {
                                    ls[last].tone = ls[j].tone;
                                    ls[j].tone = 0;
                                }
                                break;
                            }
                        }
                    }
                    if vcount >= 3 && (fv..=last).all(|j| ls[j].is_vowel) {
                        for j in (fv..last).rev() {
                            if ls[j].tone != 0 {
                                let t = ls[j].tone;
                                ls[j].tone = 0;
                                let target = if ls[fv].c == 'u'
                                    && fv + 2 <= last
                                    && ls[fv + 1].c == 'y'
                                    && ls[fv + 2].c == 'e'
                                {
                                    fv + 2
                                } else {
                                    fv + 1
                                };
                                if target <= last && target != j {
                                    ls[target].tone = t;
                                } else {
                                    ls[j].tone = t;
                                }
                                break;
                            }
                        }
                    }
                }
            } else if lc != 'w' {
                if let Some(end) = last_vowel_index(&ls) {
                    if end + 1 < ls.len() {
                        let fv = (0..end).find(|&j| ls[j].is_vowel);
                        if let Some(fv) = fv {
                            if fv < end {
                                for j in (fv..end).rev() {
                                    if ls[j].tone != 0 {
                                        ls[end].tone = ls[j].tone;
                                        ls[j].tone = 0;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    emit(&ls)
}

pub fn convert_teip_vni(input: &str, modern: bool, short_w: bool) -> String {
    let mut ls: Vec<Letter> = Vec::with_capacity(input.len());
    let mut reverted = false;
    for ch in input.chars() {
        let lc = ch.to_ascii_lowercase();
        let upper = ch.is_ascii_uppercase();
        if reverted {
            ls.push(Letter::new(ch, upper));
            continue;
        }

        // ── VNI digit rules ──
        if let Some(t) = vni_tone(lc) {
            if let Some(vi) = tone_vowel_index(&ls, modern) {
                ls[vi].tone = t;
                continue;
            }
        }
        if lc.is_ascii_digit() {
            match lc {
                '6' => {
                    if let Some(vi) = last_vowel_index(&ls) {
                        if matches!(ls[vi].c, 'a' | 'e' | 'o') {
                            ls[vi].variant = 1;
                            continue;
                        }
                    }
                }
                '7' => {
                    if let Some(vi) = last_vowel_index(&ls) {
                        if matches!(ls[vi].c, 'o' | 'u') {
                            ls[vi].variant = 2;
                            if ls[vi].c == 'o' && vi > 0 && ls[vi - 1].c == 'u' {
                                ls[vi - 1].variant = 2;
                            }
                            continue;
                        }
                    }
                }
                '8' => {
                    if let Some(vi) = last_vowel_index(&ls) {
                        if ls[vi].c == 'a' {
                            ls[vi].variant = 2;
                            continue;
                        }
                    }
                }
                '9' => {
                    if let Some(di) = last_d_index(&ls) {
                        ls[di].is_dstroke = true;
                        if upper {
                            ls[di].upper = true;
                        }
                    }
                    continue;
                }
                '0' => {
                    continue;
                }
                _ => {} // digits 1-5 already handled by vni_tone above
            }
        }

        // ── Telex dd → đ: merge once, unmerge once, then one-shot ──
        if lc == 'd' {
            let any_d_toggled = ls.iter().any(|lt| lt.c == 'd' && lt.d_toggled);
            if !any_d_toggled && last_dstroke_index(&ls).is_none() {
                if let Some(di) = last_d_index(&ls) {
                    ls[di].is_dstroke = true;
                    if upper {
                        ls[di].upper = true;
                    }
                    continue;
                }
            } else if last_dstroke_index(&ls).is_some() {
                if let Some(di) = last_dstroke_index(&ls) {
                    ls[di].is_dstroke = false;
                    ls[di].d_toggled = true;
                    reverted = true;
                    // fall through: push literal 'd'
                }
            }
        }

        // ── Circumflex toggle: a→â (in-place, like w) ──
        if matches!(lc, 'a' | 'e' | 'o') {
            let mut consumed = false;
            if let Some(vi) = last_vowel_index(&ls) {
                let any_circ_toggled = ls[..=vi]
                    .iter()
                    .any(|lt| lt.is_vowel && lt.c == lc && lt.circ_toggled);
                if !any_circ_toggled {
                    for i in (0..=vi).rev() {
                        if ls[i].is_vowel {
                            if ls[i].c == lc {
                                if i < vi {
                                    let blocked = ls[i + 1..=vi].iter().any(|lt| {
                                        lt.is_vowel
                                            && !matches!(
                                                (lc, lt.c),
                                                ('a', 'y' | 'u') | ('e', 'u') | ('o', 'i')
                                            )
                                    });
                                    if blocked {
                                        break;
                                    }
                                }
                                if ls[i].variant == 0 {
                                    if !crossing_roof_valid(&ls, i, upper, false) {
                                        break;
                                    }
                                    ls[i].variant = 1;
                                    if upper {
                                        ls[i].upper = true;
                                    }
                                    consumed = true;
                                    break;
                                }
                                if ls[i].variant == 1 {
                                    ls[i].variant = 0;
                                    ls[i].circ_toggled = true;
                                    reverted = true;
                                    break;
                                }
                                if ls[i].variant == 2 {
                                    if !crossing_roof_valid(&ls, i, upper, true) {
                                        break;
                                    }
                                    ls[i].variant = 1;
                                    if upper {
                                        ls[i].upper = true;
                                    }
                                    if ls[i].c == 'o'
                                        && i > 0
                                        && ls[i - 1].is_vowel
                                        && ls[i - 1].c == 'u'
                                        && ls[i - 1].variant == 2
                                    {
                                        ls[i - 1].variant = 0;
                                    }
                                    consumed = true;
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
            if consumed {
                continue;
            }
        }

        if lc == 'w' {
            let hook_result = crate::vseq::try_apply_hook(&mut ls);
            match hook_result {
                crate::vseq::HookResult::Applied => {
                    reposition_tone_after_hook(&mut ls, modern);
                    continue;
                }
                crate::vseq::HookResult::ToggledOff => {
                    reposition_tone_after_hook(&mut ls, modern);
                    reverted = true;
                }
                crate::vseq::HookResult::ToggledToLiteral => {
                    reverted = true;
                    continue;
                }
                crate::vseq::HookResult::NotApplicable => {
                    if crate::vseq::try_insert_horn(&mut ls, short_w, upper) {
                        continue;
                    }
                }
            }
        }

        // ── Telex tone marks ──
        if let Some(t) = telex_tone(lc) {
            if let Some(vi) = tone_vowel_index(&ls, modern) {
                if ls[vi].tone == t {
                    ls[vi].tone = 0;
                    reverted = true;
                } else {
                    ls[vi].tone = t;
                    continue;
                }
            }
        }
        if lc == 'z' {
            if let Some(vi) = tone_vowel_index(&ls, modern) {
                if ls[vi].tone != 0 {
                    ls[vi].tone = 0;
                    continue;
                }
            }
        }

        ls.push(Letter::new(ch, upper));
        // Tone reassignment: when a new vowel is pushed, move tone if the
        // vowel structure has changed (gi/qu digraph, triphthong expansion).
        if ls.len() >= 2 {
            let last = ls.len() - 1;
            if ls[last].is_vowel {
                let first_vowel = (0..last).find(|&j| ls[j].is_vowel);
                if let Some(fv) = first_vowel {
                    let vcount = last - fv + 1;
                    // gi/qu digraph: move tone from digraph vowel to new vowel
                    if vcount >= 2 {
                        for j in (fv..last).rev() {
                            if ls[j].tone != 0 {
                                let move_tone = (ls[j].c == 'i' && j > 0 && ls[j - 1].c == 'g')
                                    || (ls[j].c == 'u' && j > 0 && ls[j - 1].c == 'q');
                                if move_tone {
                                    ls[last].tone = ls[j].tone;
                                    ls[j].tone = 0;
                                }
                                break;
                            }
                        }
                    }
                    // Triphthong expansion: 2→3+ vowels, move tone to
                    // correct position (middle vowel, or 3rd for uyê).
                    if vcount >= 3 {
                        for j in (fv..last).rev() {
                            if ls[j].tone != 0 {
                                let t = ls[j].tone;
                                ls[j].tone = 0;
                                let target = if ls[fv].c == 'u'
                                    && fv + 2 <= last
                                    && ls[fv + 1].c == 'y'
                                    && ls[fv + 2].c == 'e'
                                {
                                    fv + 2
                                } else {
                                    fv + 1
                                };
                                if target <= last && target != j {
                                    ls[target].tone = t;
                                } else {
                                    ls[j].tone = t;
                                }
                                break;
                            }
                        }
                    }
                }
            } else {
                // Trailing consonant: closed syllable → tone must be on
                // last vowel.
                if let Some(end) = last_vowel_index(&ls) {
                    if end + 1 < ls.len() {
                        let fv = (0..end).find(|&j| ls[j].is_vowel);
                        if let Some(fv) = fv {
                            if fv < end {
                                for j in (fv..end).rev() {
                                    if ls[j].tone != 0 {
                                        ls[end].tone = ls[j].tone;
                                        ls[j].tone = 0;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    emit(&ls)
}

/// VIQR (Vietnamese Quoted-Readable) conversion.
///
/// Uses ASCII punctuation as diacritic markers:
///   ^ circumflex, ( breve, + horn (modifiers)
///   ' sac, ` huyen, ? hoi, ~ nga, . nang (tones)
///   dd -> đ d-stroke
pub fn convert_viqr(input: &str) -> String {
    let mut ls: Vec<Letter> = Vec::with_capacity(input.len());
    for ch in input.chars() {
        let lc = ch.to_ascii_lowercase();
        let n = ls.len();

        // Modifier characters: apply to last vowel
        match ch {
            '^' | '(' | '+' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    match (ch, ls[vi].c) {
                        ('^', 'a' | 'e' | 'o') => {
                            ls[vi].variant = 1;
                            continue;
                        }
                        ('(', 'a') => {
                            ls[vi].variant = 2;
                            continue;
                        }
                        ('+', 'o' | 'u') => {
                            ls[vi].variant = 2;
                            continue;
                        }
                        _ => {} // illegal combination: fall through to literal
                    }
                }
                // Fall through: treat as literal character
                ls.push(Letter {
                    c: ch,
                    is_vowel: false,
                    variant: 0,
                    tone: 0,
                    is_dstroke: false,
                    upper: false,
                    from_w: false,
                    horn_propagated: false,
                    circ_toggled: false,
                    d_toggled: false,
                    horn_toggled: false,
                });
                continue;
            }
            _ => {}
        }

        // Tone characters
        match ch {
            '\'' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    ls[vi].tone = 2;
                    continue;
                }
            }
            '`' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    ls[vi].tone = 1;
                    continue;
                }
            }
            '?' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    ls[vi].tone = 3;
                    continue;
                }
            }
            '~' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    ls[vi].tone = 4;
                    continue;
                }
            }
            '.' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    ls[vi].tone = 5;
                    continue;
                }
            }
            _ => {}
        }

        // dd → đ
        if lc == 'd' && n > 0 && ls[n - 1].c == 'd' && !ls[n - 1].is_vowel && !ls[n - 1].is_dstroke
        {
            ls[n - 1].is_dstroke = true;
            if ch.is_ascii_uppercase() {
                ls[n - 1].upper = true;
            }
            continue;
        }

        ls.push(Letter::new(ch, ch.is_ascii_uppercase()));
    }
    emit(&ls)
}

pub fn convert_vni(input: &str, modern: bool, _short_w: bool) -> String {
    let mut ls: Vec<Letter> = Vec::with_capacity(input.len());
    for ch in input.chars() {
        let lc = ch.to_ascii_lowercase();
        let upper = ch.is_ascii_uppercase();
        if let Some(t) = vni_tone(lc) {
            if let Some(vi) = tone_vowel_index(&ls, modern) {
                ls[vi].tone = t;
                continue;
            }
        }
        match lc {
            '6' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    if matches!(ls[vi].c, 'a' | 'e' | 'o') {
                        ls[vi].variant = 1;
                        continue;
                    }
                }
            }
            '7' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    if matches!(ls[vi].c, 'o' | 'u') {
                        ls[vi].variant = 2;
                        if ls[vi].c == 'o' && vi > 0 && ls[vi - 1].c == 'u' {
                            ls[vi - 1].variant = 2;
                        }
                        continue;
                    }
                }
            }
            '8' => {
                if let Some(vi) = last_vowel_index(&ls) {
                    if ls[vi].c == 'a' {
                        ls[vi].variant = 2;
                        continue;
                    }
                }
            }
            '9' => {
                if let Some(di) = last_d_index(&ls) {
                    ls[di].is_dstroke = true;
                    if upper {
                        ls[di].upper = true;
                    }
                }
                continue;
            }
            '0' => {
                continue;
            }
            _ => {}
        }
        if lc.is_ascii_alphabetic() || (lc.is_ascii_digit() && !ls.is_empty()) {
            ls.push(Letter::new(ch, upper));
        }
    }
    emit(&ls)
}
