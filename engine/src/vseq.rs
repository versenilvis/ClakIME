use crate::engine::Letter;

#[derive(PartialEq, Eq)]
pub enum HookResult {
    /// Hook was applied to vowel(s); w key is consumed
    Applied,
    /// Hook was removed (toggled off); w key falls through as literal
    ToggledOff,
    /// Standalone ư (from bare w) was rewritten to a literal w; w consumed
    ToggledToLiteral,
    /// No hookable vowel found; try phase 2 (insert standalone ư)
    NotApplicable,
}

struct VSeq {
    base: [char; 3],
    len: u8,
    hook_mask: u8,
}

static VSEQ_TABLE: &[VSeq] = &[
    VSeq {
        base: ['u', 'o', 'i'],
        len: 3,
        hook_mask: 0b011,
    },
    VSeq {
        base: ['u', 'o', 'u'],
        len: 3,
        hook_mask: 0b011,
    },
    VSeq {
        base: ['u', 'o', '\0'],
        len: 2,
        hook_mask: 0b11,
    },
    VSeq {
        base: ['u', 'a', '\0'],
        len: 2,
        hook_mask: 0b01,
    },
    VSeq {
        base: ['u', 'i', '\0'],
        len: 2,
        hook_mask: 0b01,
    },
    VSeq {
        base: ['u', 'y', '\0'],
        len: 2,
        hook_mask: 0b01,
    },
    VSeq {
        base: ['u', 'e', '\0'],
        len: 2,
        hook_mask: 0b01,
    },
    VSeq {
        base: ['o', 'a', '\0'],
        len: 2,
        hook_mask: 0b10,
    },
    VSeq {
        base: ['o', 'e', '\0'],
        len: 2,
        hook_mask: 0b01,
    },
    VSeq {
        base: ['a', '\0', '\0'],
        len: 1,
        hook_mask: 0b01,
    },
    VSeq {
        base: ['o', '\0', '\0'],
        len: 1,
        hook_mask: 0b01,
    },
    VSeq {
        base: ['u', '\0', '\0'],
        len: 1,
        hook_mask: 0b01,
    },
];

fn vowel_cluster(ls: &[Letter]) -> Option<(usize, usize)> {
    let end = ls.iter().rposition(|lt| lt.is_vowel)?;
    let mut start = end;
    while start > 0 && ls[start - 1].is_vowel {
        start -= 1;
    }
    // skip gi/qu digraph vowels
    if start > 0
        && ((ls[start].c == 'u' && ls[start - 1].c == 'q')
            || (ls[start].c == 'i' && ls[start - 1].c == 'g'))
    {
        start += 1;
    }
    if start > end {
        return None;
    }
    Some((start, end - start + 1))
}

fn lookup_vseq(base: &[char]) -> Option<&'static VSeq> {
    VSEQ_TABLE
        .iter()
        .find(|vs| vs.len as usize == base.len() && (0..base.len()).all(|i| vs.base[i] == base[i]))
}

pub(crate) fn try_apply_hook(ls: &mut Vec<Letter>) -> HookResult {
    let (vstart, vcount) = match vowel_cluster(ls) {
        Some(vc) => vc,
        None => return HookResult::NotApplicable,
    };

    let base: Vec<char> = (vstart..vstart + vcount).map(|i| ls[i].c).collect();

    let vs = match lookup_vseq(&base) {
        Some(vs) => vs,
        None => {
            for i in (vstart..vstart + vcount).rev() {
                if matches!(ls[i].c, 'a' | 'o' | 'u') {
                    return apply_single_hook(ls, i);
                }
            }
            return HookResult::NotApplicable;
        }
    };

    let all_hooked = (0..vs.len as usize)
        .filter(|&i| (vs.hook_mask >> i) & 1 != 0)
        .all(|i| ls[vstart + i].variant == 2);

    if all_hooked {
        let any_propagated = (0..vs.len as usize)
            .filter(|&i| (vs.hook_mask >> i) & 1 != 0)
            .any(|i| ls[vstart + i].horn_propagated);
        if any_propagated {
            for i in 0..vs.len as usize {
                if (vs.hook_mask >> i) & 1 != 0 {
                    ls[vstart + i].horn_propagated = false;
                }
            }
            return HookResult::Applied;
        }
        if vs.len == 1 && vs.base[0] == 'u' && ls[vstart].variant == 2 && ls[vstart].from_w {
            ls[vstart].c = 'w';
            ls[vstart].is_vowel = false;
            ls[vstart].from_w = false;
            return HookResult::ToggledToLiteral;
        }
        for i in 0..vs.len as usize {
            if (vs.hook_mask >> i) & 1 != 0 {
                ls[vstart + i].variant = 0;
                ls[vstart + i].from_w = true;
                ls[vstart + i].horn_propagated = false;
                ls[vstart + i].horn_toggled = true;
            }
        }
        HookResult::ToggledOff
    } else {
        let any_toggled = (0..vs.len as usize)
            .filter(|&i| (vs.hook_mask >> i) & 1 != 0)
            .any(|i| ls[vstart + i].horn_toggled);
        if any_toggled {
            return HookResult::ToggledOff;
        }

        let effective_mask = vs.hook_mask;

        for i in 0..vs.len as usize {
            if (effective_mask >> i) & 1 != 0 && ls[vstart + i].variant != 2 {
                ls[vstart + i].variant = 2;
                ls[vstart + i].from_w = false;
            }
        }
        HookResult::Applied
    }
}

fn apply_single_hook(ls: &mut Vec<Letter>, vi: usize) -> HookResult {
    let c = ls[vi].c;
    match c {
        'a' => {
            if ls[vi].variant == 2 {
                ls[vi].variant = 0;
                ls[vi].from_w = true;
                HookResult::ToggledOff
            } else if ls[vi].from_w && ls[vi].variant == 0 {
                HookResult::ToggledOff
            } else {
                ls[vi].variant = 2;
                ls[vi].from_w = false;
                HookResult::Applied
            }
        }
        'o' => {
            if ls[vi].variant == 2 {
                ls[vi].variant = 0;
                ls[vi].from_w = true;
                if vi > 0 && ls[vi - 1].c == 'u' && ls[vi - 1].variant == 2 {
                    ls[vi - 1].variant = 0;
                    ls[vi - 1].from_w = true;
                }
                HookResult::ToggledOff
            } else if ls[vi].from_w && ls[vi].variant == 0 {
                HookResult::ToggledOff
            } else {
                ls[vi].variant = 2;
                ls[vi].from_w = false;
                if vi > 0 && ls[vi - 1].c == 'u' && ls[vi - 1].variant == 0 {
                    ls[vi - 1].variant = 2;
                    ls[vi - 1].from_w = false;
                }
                HookResult::Applied
            }
        }
        'u' => {
            let mut first = vi;
            while first > 0 && ls[first - 1].c == 'u' && ls[first - 1].is_vowel {
                first -= 1;
            }
            if ls[first].variant == 2 {
                if ls[first].from_w {
                    ls[first].c = 'w';
                    ls[first].is_vowel = false;
                    ls[first].from_w = false;
                    HookResult::ToggledToLiteral
                } else {
                    ls[first].variant = 0;
                    ls[first].from_w = true;
                    HookResult::ToggledOff
                }
            } else if ls[first].from_w && ls[first].variant == 0 {
                HookResult::ToggledOff
            } else {
                ls[first].variant = 2;
                ls[first].from_w = false;
                HookResult::Applied
            }
        }
        _ => HookResult::NotApplicable,
    }
}

pub(crate) fn try_insert_horn(ls: &mut Vec<Letter>, short_w: bool, upper: bool) -> bool {
    let has_real_vowel = ls.iter().enumerate().any(|(idx, lt)| {
        if !lt.is_vowel {
            return false;
        }
        if lt.c == 'i' && idx > 0 && ls[idx - 1].c == 'g' && !ls[idx - 1].is_vowel {
            return false;
        }
        if lt.c == 'u' && idx > 0 && ls[idx - 1].c == 'q' && !ls[idx - 1].is_vowel {
            return false;
        }
        true
    });

    let is_unit_w = ls.last().is_some_and(|lt| lt.c.is_ascii_digit());
    if short_w
        && !is_unit_w
        && !has_real_vowel
        && !ls.last().is_some_and(|lt| lt.c == 'w' && !lt.is_vowel)
    {
        ls.push(Letter::new_standalone_u(upper));
        return true;
    }
    false
}

impl Letter {
    pub fn new_standalone_u(upper: bool) -> Self {
        Letter {
            c: 'u',
            is_vowel: true,
            variant: 2,
            tone: 0,
            is_dstroke: false,
            upper,
            from_w: true,
            horn_propagated: false,
            circ_toggled: false,
            d_toggled: false,
            horn_toggled: false,
        }
    }
}
