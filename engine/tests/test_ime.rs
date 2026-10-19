use clak_engine::*;
use std::ffi::CStr;

#[test]
fn test_surrounding_dd_to_d_stroke() {
    let mut ctx = ClakContext::new(Method::Telex);
    let action = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
    assert_eq!(action.action_type, ACTION_UINPUT_REPLACE);
    assert_eq!(action.delete_count, 1);
    let commit_text = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
    assert_eq!(commit_text, "đ");
}

#[test]
fn test_address_bar_autocomplete_selection() {
    // cursor=1, anchor=10: user typed 'd', autocomplete showed 'douyin.com'
    // we must NOT seed from surrounding text (that would double-count the 'd')
    // so raw_buffer = "d" only, transform = "d", no replacement → forward
    // the autocomplete fix happens on the second 'd': see test_address_bar_autocomplete_flow
    let mut ctx = ClakContext::new(Method::Telex);
    let action = ctx.process_key(b'd' as u32, "d", false, Some("douyin.com"), 1, 10);
    // engine did not seed from surrounding text, so 'd' alone stays as 'd' → forward
    assert_eq!(action.action_type, ACTION_FORWARD);
}

#[test]
fn test_normal_english_key_forwards() {
    let mut ctx = ClakContext::new(Method::Telex);
    let action = ctx.process_key(b'k' as u32, "k", false, Some("hello "), 6, 6);
    assert_eq!(action.action_type, ACTION_FORWARD);
}

#[test]
fn test_address_bar_stale_surrounding() {
    let mut ctx = ClakContext::new(Method::Telex);
    let act1 = ctx.process_key(b'd' as u32, "d", false, Some("\n"), 0, 0);
    assert_eq!(act1.action_type, ACTION_FORWARD);
    let act2 = ctx.process_key(b'd' as u32, "d", false, Some("\n"), 0, 0);
    assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);
    assert_eq!(act2.delete_count, 1);
    let commit = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
    assert_eq!(commit, "đ");
}

#[test]
fn test_address_bar_autocomplete_flow() {
    let mut ctx = ClakContext::new(Method::Telex);
    let act1 = ctx.process_key(b'd' as u32, "d", false, Some("\n"), 0, 0);
    assert_eq!(act1.action_type, ACTION_FORWARD);
    let act2 = ctx.process_key(b'd' as u32, "d", false, Some("discord.com"), 1, 11);
    assert_eq!(act2.action_type, ACTION_ADDRESS_BAR_FIX);
    assert_eq!(act2.delete_count, 1);
    let commit = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
    assert_eq!(commit, "đ");
}

#[test]
fn test_address_bar_typing_over_selected_url() {
    let mut ctx = ClakContext::new(Method::Telex);
    let act1 = ctx.process_key(
        b'd' as u32,
        "d",
        false,
        Some("https://www.facebook.com"),
        0,
        24,
    );
    assert_eq!(act1.action_type, ACTION_FORWARD);
    let act2 = ctx.process_key(
        b'd' as u32,
        "d",
        false,
        Some("https://www.facebook.com"),
        0,
        24,
    );
    assert_eq!(act2.action_type, ACTION_ADDRESS_BAR_FIX);
    assert_eq!(act2.delete_count, 1);
    let commit = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
    assert_eq!(commit, "đ");
}

#[test]
fn test_address_bar_delete_and_retype() {
    let mut ctx = ClakContext::new(Method::Telex);
    let act1 = ctx.process_key(b'd' as u32, "d", false, Some("\n"), 0, 0);
    assert_eq!(act1.action_type, ACTION_FORWARD);
    let act2 = ctx.process_key(b'd' as u32, "d", false, Some("discord.com"), 1, 11);
    assert_eq!(act2.action_type, ACTION_ADDRESS_BAR_FIX);
    assert_eq!(act2.delete_count, 1);

    // dismiss autocomplete suggestion
    let act_bs1 = ctx.process_key(0xff08, "", false, Some("đangcap.vn"), 1, 10);
    assert_eq!(act_bs1.action_type, ACTION_FORWARD);

    // delete 'đ' character
    let act_bs2 = ctx.process_key(0xff08, "", false, Some("đ"), 1, 1);
    assert_eq!(act_bs2.action_type, ACTION_FORWARD);

    // address bar is empty, type 'd' again
    let act3 = ctx.process_key(b'd' as u32, "d", false, Some(""), 0, 0);
    // must forward 'd', not become 'đ'
    assert_eq!(act3.action_type, ACTION_FORWARD);
}

#[test]
fn test_address_bar_autocomplete_with_spaces_query() {
    let mut ctx = ClakContext::new(Method::Telex);
    // user types 'c'
    let a1 = ctx.process_key(b'c' as u32, "c", false, Some(""), 0, 0);
    assert_eq!(a1.action_type, ACTION_FORWARD);

    // browser autocompletes 'claude.ai'(1..9), user types 'a'
    let a2 = ctx.process_key(b'a' as u32, "a", false, Some("claude.ai"), 1, 9);
    assert_eq!(a2.action_type, ACTION_FORWARD);

    // browser autocompletes search suggestion 'cach check hạn gg pro'(2..21), user types 'n'
    let a3 = ctx.process_key(
        b'n' as u32,
        "n",
        false,
        Some("cach check hạn gg pro"),
        2,
        21,
    );
    assert_eq!(a3.action_type, ACTION_FORWARD);
    assert_eq!(ctx.raw_buffer, "can");

    // browser replaced selection with 'can', user types 'h'
    let a4 = ctx.process_key(b'h' as u32, "h", false, Some("can"), 3, 3);
    assert_eq!(a4.action_type, ACTION_FORWARD);
    assert_eq!(ctx.raw_buffer, "canh");

    // browser autocompletes 'canh chua ca loc'(4..17), user types 's'
    let a5 = ctx.process_key(b's' as u32, "s", false, Some("canh chua ca loc"), 4, 17);
    assert_eq!(a5.action_type, ACTION_ADDRESS_BAR_FIX);
    let commit5 = unsafe { CStr::from_ptr(a5.commit_str).to_str().unwrap() };
    assert_eq!(commit5, "ánh");

    // also verify without autocomplete on 's'
    let mut ctx2 = ClakContext::new(Method::Telex);
    ctx2.process_key(b'c' as u32, "c", false, Some(""), 0, 0);
    ctx2.process_key(b'a' as u32, "a", false, Some("claude.ai"), 1, 9);
    ctx2.process_key(
        b'n' as u32,
        "n",
        false,
        Some("cach check hạn gg pro"),
        2,
        21,
    );
    ctx2.process_key(b'h' as u32, "h", false, Some("can"), 3, 3);
    let a5_no_auto = ctx2.process_key(b's' as u32, "s", false, Some("canh"), 4, 4);
    assert_eq!(a5_no_auto.action_type, ACTION_REPLACE);
    let commit_no_auto = unsafe { CStr::from_ptr(a5_no_auto.commit_str).to_str().unwrap() };
    assert_eq!(commit_no_auto, "ánh");
}

#[test]
fn test_backspace_and_retype_with_stale_surrounding() {
    let mut ctx = ClakContext::new(Method::Telex);
    // step 1: type 'dd' -> 'đ'
    ctx.process_key(b'd' as u32, "d", false, Some(""), 0, 0);
    let act2 = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
    assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);

    // step 2: backspace deletes 'đ'
    let act_bs = ctx.process_key(0xff08, "", false, Some("đ"), 1, 1);
    assert_eq!(act_bs.action_type, ACTION_FORWARD);

    // step 3: retype 'd' while surrounding is still stale 'đ'(1, 1)
    let act3 = ctx.process_key(b'd' as u32, "d", false, Some("đ"), 1, 1);
    assert_eq!(act3.action_type, ACTION_FORWARD);
    assert_eq!(ctx.raw_buffer, "d");
    assert_eq!(ctx.last_composed, "d");

    // step 4: type second 'd' -> should become 'đ'
    let act4 = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
    assert_eq!(act4.action_type, ACTION_UINPUT_REPLACE);
    let commit4 = unsafe { CStr::from_ptr(act4.commit_str).to_str().unwrap() };
    assert_eq!(commit4, "đ");
    assert_eq!(ctx.raw_buffer, "dd");
    assert_eq!(ctx.last_composed, "đ");
}

#[test]
fn test_user_scenario_dd_del_dddddd_del_dd() {
    let mut ctx = ClakContext::new(Method::Telex);
    // step 1: dd -> đ
    let _ = ctx.process_key(b'd' as u32, "d", false, Some(""), 0, 0);
    let _ = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
    assert_eq!(ctx.last_composed, "đ");
    assert_eq!(ctx.raw_buffer, "dd");

    // step 2: backspace đ
    let _ = ctx.process_key(0xff08, "", false, Some("đ"), 1, 1);
    assert_eq!(ctx.last_composed, "");
    assert_eq!(ctx.raw_buffer, "");

    // step 3: type dddddd
    for _ in "dddddd".chars() {
        let _ = ctx.process_key(b'd' as u32, "d", false, None, 0, 0);
    }
    assert_eq!(ctx.last_composed, "ddddd");
    assert_eq!(ctx.raw_buffer, "dddddd");

    // step 4: backspace all
    let count = ctx.last_composed.chars().count();
    for _ in 0..count {
        let _ = ctx.process_key(0xff08, "", false, None, 0, 0);
    }
    assert_eq!(ctx.last_composed, "");
    assert_eq!(ctx.raw_buffer, "");

    // step 5: type dd -> đ cleanly
    let a_d1 = ctx.process_key(b'd' as u32, "d", false, None, 0, 0);
    assert_eq!(a_d1.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "d");
    assert_eq!(ctx.raw_buffer, "d");

    let a_d2 = ctx.process_key(b'd' as u32, "d", false, None, 0, 0);
    assert_eq!(a_d2.action_type, ACTION_UINPUT_REPLACE);
    assert_eq!(ctx.last_composed, "đ");
    assert_eq!(ctx.raw_buffer, "dd");
}

#[test]
fn test_surrounding_vietnamese_word_progression() {
    let mut ctx = ClakContext::new(Method::Telex);
    let action = ctx.process_key(b'j' as u32, "j", false, Some("viêt"), 4, 4);
    let commit_text = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
    assert_eq!(action.action_type, ACTION_UINPUT_REPLACE);
    assert_eq!(action.delete_count, 2);
    assert_eq!(commit_text, "ệt");
}

fn simulate_typing(input: &str) -> String {
    let mut ctx = ClakContext::new(Method::Telex);
    let mut doc = String::new();
    for ch in input.chars() {
        let key_str = ch.to_string();
        let cur = doc.chars().count();
        let action = ctx.process_key(ch as u32, &key_str, false, Some(&doc), cur, cur);
        match action.action_type {
            ACTION_FORWARD => {
                doc.push(ch);
            }
            ACTION_COMMIT => {
                let s = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
                doc.push_str(s);
            }
            ACTION_REPLACE
                | ACTION_REPLACE_SURROUNDING
                | ACTION_ADDRESS_BAR_FIX => {
                let del = action.delete_count;
                let s = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
                let mut doc_chars: Vec<char> = doc.chars().collect();
                for _ in 0..del {
                    doc_chars.pop();
                }
                doc = doc_chars.into_iter().collect();
                doc.push_str(s);
            }
            _ => {}
        }
    }
    doc
}

fn simulate_typing_fallback(input: &str) -> String {
    let mut ctx = ClakContext::new(Method::Telex);
    let mut doc = String::new();
    for ch in input.chars() {
        let key_str = ch.to_string();
        let action = ctx.process_key(ch as u32, &key_str, false, None, 0, 0);
        match action.action_type {
            ACTION_FORWARD => {
                doc.push(ch);
            }
            ACTION_COMMIT => {
                let s = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
                doc.push_str(s);
            }
            ACTION_REPLACE
                | ACTION_REPLACE_SURROUNDING
                | ACTION_ADDRESS_BAR_FIX => {
                let del = action.delete_count;
                let s = unsafe { CStr::from_ptr(action.commit_str).to_str().unwrap() };
                let mut doc_chars: Vec<char> = doc.chars().collect();
                for _ in 0..del {
                    doc_chars.pop();
                }
                doc = doc_chars.into_iter().collect();
                doc.push_str(s);
            }
            _ => {}
        }
    }
    doc
}

#[test]
fn test_full_vietnamese_typing() {
    assert_eq!(simulate_typing("dd"), "đ");
    assert_eq!(simulate_typing("tieengs"), "tiếng");
    assert_eq!(simulate_typing("vieetj"), "việt");
    assert_eq!(simulate_typing("chaof"), "chào");
    assert_eq!(simulate_typing("nguwowif"), "người");
    assert_eq!(simulate_typing("dduwowcj"), "được");
    assert_eq!(simulate_typing("khoong"), "không");
    assert_eq!(
        simulate_typing("chaof banj tooi laf nguwowif vieetj nam"),
        "chào bạn tôi là người việt nam"
    );
    assert_eq!(simulate_typing("test"), "tét");
    assert_eq!(simulate_typing("assssssssss"), "asssssssss");
}

#[test]
fn test_fallback_vietnamese_typing() {
    assert_eq!(simulate_typing_fallback("dd"), "đ");
    assert_eq!(simulate_typing_fallback("tieengs"), "tiếng");
    assert_eq!(simulate_typing_fallback("vieetj"), "việt");
    assert_eq!(simulate_typing_fallback("chaof"), "chào");
    assert_eq!(simulate_typing_fallback("nguwowif"), "người");
    assert_eq!(simulate_typing_fallback("dduwowcj"), "được");
    assert_eq!(simulate_typing_fallback("khoong"), "không");
    assert_eq!(simulate_typing_fallback("ddaay"), "đây");
    assert_eq!(simulate_typing_fallback("booj"), "bộ");
    assert_eq!(simulate_typing_fallback("mooj"), "mộ");
    assert_eq!(
        simulate_typing_fallback("chaof banj tooi laf nguwowif vieetj nam"),
        "chào bạn tôi là người việt nam"
    );
    assert_eq!(simulate_typing_fallback("test"), "tét");
    assert_eq!(simulate_typing_fallback("tesst"), "test");
    assert_eq!(simulate_typing_fallback("as"), "á");
    assert_eq!(simulate_typing_fallback("ass"), "as");
    assert_eq!(simulate_typing_fallback("asss"), "ass");
    assert_eq!(simulate_typing_fallback("assss"), "asss");
    assert_eq!(simulate_typing_fallback("assssssssss"), "asssssssss");
}

#[test]
fn test_chuns_progression() {
    let mut ctx = ClakContext::new(Method::Telex);
    let keys = ["c", "h", "u", "n", "s"];
    for k in keys {
        ctx.process_key(k.chars().next().unwrap() as u32, k, false, None, 0, 0);
    }
    assert_eq!(ctx.last_composed, "chún");
    ctx.process_key('g' as u32, "g", false, None, 0, 0);
    assert_eq!(ctx.last_composed, "chúng");
}

#[test]
fn test_twitter_placeholder_typing_is() {
    let mut ctx = ClakContext::new(Method::Telex);
    // Twitter/Facebook modal opens with placeholder or button text in surrounding
    let act1 = ctx.process_key(b'i' as u32, "i", false, Some("Có gì đang xảy ra?\n"), 0, 0);
    assert_eq!(act1.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "i");

    let act2 = ctx.process_key(b's' as u32, "s", false, Some("i"), 1, 1);
    assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);
    assert_eq!(act2.delete_count, 1);
    let commit2 = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
    assert_eq!(commit2, "í");

    // type second s to toggle to english 'is'
    let act3 = ctx.process_key(b's' as u32, "s", false, Some("í"), 1, 1);
    assert_eq!(act3.action_type, ACTION_UINPUT_REPLACE);
    assert_eq!(act3.delete_count, 1);
    let commit3 = unsafe { CStr::from_ptr(act3.commit_str).to_str().unwrap() };
    assert_eq!(commit3, "is");
}

#[test]
fn test_multiline_editor_no_false_autocomplete() {
    let mut ctx = ClakContext::new(Method::Telex);
    // multiline editor with selection (cursor != anchor) should NOT trigger address bar autocomplete
    let act = ctx.process_key(b'i' as u32, "i", false, Some("Line 1\nLine 2"), 0, 6);
    assert_eq!(act.action_type, ACTION_FORWARD);
}

#[test]
fn test_twitter_draftjs_trailing_newline_is() {
    let mut ctx = ClakContext::new(Method::Telex);
    // simulate Twitter Draft.js composer where empty box has '\n' and typed word has trailing '\n'
    let act1 = ctx.process_key(b'i' as u32, "i", false, Some("\n"), 0, 0);
    assert_eq!(act1.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "i");

    // key 's' arrives with surrounding "i\n" and cursor=1
    let act2 = ctx.process_key(b's' as u32, "s", false, Some("i\n"), 1, 1);
    assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);
    assert_eq!(act2.delete_count, 1);
    let commit2 = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
    assert_eq!(commit2, "í");
}

#[test]
fn test_twitter_draftjs_reseed_uaj_to_ua_with_dot() {
    let mut ctx = ClakContext::new(Method::Telex);
    // even if reset happened between u and a:
    ctx.reset();
    // user presses 'a' with surrounding "u\n"
    let act1 = ctx.process_key(b'a' as u32, "a", false, Some("u\n"), 1, 1);
    assert_eq!(act1.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "ua");

    // user presses 'j' with surrounding "ua\n"
    let act2 = ctx.process_key(b'j' as u32, "j", false, Some("ua\n"), 2, 2);
    assert_eq!(act2.action_type, ACTION_UINPUT_REPLACE);
    assert_eq!(act2.delete_count, 2);
    let commit2 = unsafe { CStr::from_ptr(act2.commit_str).to_str().unwrap() };
    assert_eq!(commit2, "ụa");
}

#[test]
fn test_typing_over_selected_word_in_sentence() {
    let mut ctx = ClakContext::new(Method::Telex);
    let text = "Fcitx5 and Wayland";
    // user selects "and" (indices 7..10) and types "và" (v, a, f)
    let a1 = ctx.process_key(b'v' as u32, "v", false, Some(text), 7, 10);
    assert_eq!(a1.action_type, ACTION_FORWARD);

    // editor replaced "and" with "v", cursor at 8, anchor at 8
    let a2 = ctx.process_key(b'a' as u32, "a", false, Some("Fcitx5 v Wayland"), 8, 8);
    assert_eq!(a2.action_type, ACTION_FORWARD);

    // typing 'f' to transform "va" -> "và"
    let a3 = ctx.process_key(b'f' as u32, "f", false, Some("Fcitx5 va Wayland"), 9, 9);
    assert_eq!(a3.action_type, ACTION_REPLACE);
    assert_eq!(a3.delete_count, 1);
    let commit3 = unsafe { CStr::from_ptr(a3.commit_str).to_str().unwrap() };
    assert_eq!(commit3, "à");
}

#[test]
fn test_bracket_shortcuts() {
    let mut ctx = ClakContext::new(Method::Telex);
    let a1 = ctx.process_key(b'[' as u32, "[", false, None, 0, 0);
    assert_eq!(a1.action_type, ACTION_REPLACE);
    let commit1 = unsafe { CStr::from_ptr(a1.commit_str).to_str().unwrap() };
    assert_eq!(commit1, "ơ");

    let a2 = ctx.process_key(b'[' as u32, "[", false, None, 0, 0);
    assert_eq!(a2.action_type, ACTION_REPLACE);
    assert_eq!(a2.delete_count, 1);
    let commit2 = unsafe { CStr::from_ptr(a2.commit_str).to_str().unwrap() };
    assert_eq!(commit2, "[");
}

#[test]
fn test_macro_expansion() {
    let mut ctx = ClakContext::new(Method::Telex);
    ctx.macros.insert("vn".to_string(), "Việt Nam".to_string());
    ctx.process_key(b'v' as u32, "v", false, None, 0, 0);
    ctx.process_key(b'n' as u32, "n", false, None, 1, 1);
    let act = ctx.process_key(0x20, " ", false, None, 2, 2);
    assert_eq!(act.action_type, ACTION_REPLACE);
    assert_eq!(act.delete_count, 2);
    let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
    assert_eq!(commit, "Việt Nam ");
}

#[test]
fn test_macro_case_uppercase() {
    let mut ctx = ClakContext::new(Method::Telex);
    ctx.macros.insert("vn".to_string(), "Việt Nam".to_string());
    ctx.process_key(b'V' as u32, "V", false, None, 0, 0);
    ctx.process_key(b'N' as u32, "N", false, None, 1, 1);
    let act = ctx.process_key(0x20, " ", false, None, 2, 2);
    assert_eq!(act.action_type, ACTION_REPLACE);
    assert_eq!(act.delete_count, 2);
    let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
    assert_eq!(commit, "VIỆT NAM ");
}

#[test]
fn test_macro_case_capitalized() {
    let mut ctx = ClakContext::new(Method::Telex);
    ctx.macros.insert("ko".to_string(), "không".to_string());
    ctx.process_key(b'K' as u32, "K", false, None, 0, 0);
    ctx.process_key(b'o' as u32, "o", false, None, 1, 1);
    let act = ctx.process_key(0x20, " ", false, None, 2, 2);
    assert_eq!(act.action_type, ACTION_REPLACE);
    assert_eq!(act.delete_count, 2);
    let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
    assert_eq!(commit, "Không ");
}

#[test]
fn test_macro_enter_trigger() {
    let mut ctx = ClakContext::new(Method::Telex);
    ctx.macros.insert("vn".to_string(), "Việt Nam".to_string());
    ctx.process_key(b'v' as u32, "v", false, None, 0, 0);
    ctx.process_key(b'n' as u32, "n", false, None, 1, 1);
    let act = ctx.process_key(0xff0d, "\n", false, None, 2, 2);
    assert_eq!(act.action_type, ACTION_REPLACE);
    assert_eq!(act.delete_count, 2);
    let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
    assert_eq!(commit, "Việt Nam\n");
}

#[test]
fn test_macro_raw_buffer_trigger() {
    let mut ctx = ClakContext::new(Method::Telex);
    ctx.macros.insert("ddc".to_string(), "được".to_string());
    ctx.process_key(b'd' as u32, "d", false, None, 0, 0);
    ctx.process_key(b'd' as u32, "d", false, None, 1, 1);
    ctx.process_key(b'c' as u32, "c", false, None, 2, 2);
    let act = ctx.process_key(0x20, " ", false, None, 2, 2);
    assert_eq!(act.action_type, ACTION_REPLACE);
    assert_eq!(act.delete_count, 2);
    let commit = unsafe { CStr::from_ptr(act.commit_str).to_str().unwrap() };
    assert_eq!(commit, "được ");
}

#[test]
fn test_delete_all_and_retype() {
    let mut ctx = ClakContext::new(Method::Telex);
    // step 1: type "in" -> forwards 'i', forwards 'n'
    ctx.process_key(b'i' as u32, "i", false, Some("\n"), 0, 0);
    ctx.process_key(b'n' as u32, "n", false, Some("i\n"), 1, 1);
    // step 2: type 's' -> replaces to "ín"
    let act_s = ctx.process_key(b's' as u32, "s", false, Some("in\n"), 2, 2);
    assert_eq!(act_s.action_type, ACTION_REPLACE);
    assert_eq!(ctx.last_composed, "ín");

    // step 3: user deletes everything in the editor!
    // editor text is now "\n", cursor at 0
    // user types 'i' again
    let act_i = ctx.process_key(b'i' as u32, "i", false, Some("\n"), 0, 0);
    assert_eq!(act_i.action_type, ACTION_FORWARD);
    assert_eq!(ctx.raw_buffer, "i");
    assert_eq!(ctx.last_composed, "i");

    // user types 'n'
    let act_n = ctx.process_key(b'n' as u32, "n", false, Some("i\n"), 1, 1);
    assert_eq!(act_n.action_type, ACTION_FORWARD);
    assert_eq!(ctx.raw_buffer, "in");
    assert_eq!(ctx.last_composed, "in");

    // user types 's'
    let act_s2 = ctx.process_key(b's' as u32, "s", false, Some("in\n"), 2, 2);
    assert_eq!(act_s2.action_type, ACTION_REPLACE);
    assert_eq!(ctx.last_composed, "ín");
}

#[test]
fn test_is_composing_state() {
    let mut ctx = ClakContext::new(Method::Telex);
    assert!(!unsafe { clak_is_composing(&ctx as *const ClakContext) });

    ctx.process_key(b't' as u32, "t", false, None, 0, 0);
    assert!(unsafe { clak_is_composing(&ctx as *const ClakContext) });

    ctx.reset();
    assert!(!unsafe { clak_is_composing(&ctx as *const ClakContext) });
}

#[test]
fn test_rapid_typing_lagging_surrounding_dduowcj() {
    let mut ctx = ClakContext::new(Method::Telex);
    // key 1: 'd' -> forwards 'd'
    let act1 = ctx.process_key(b'd' as u32, "d", false, Some(""), 0, 0);
    assert_eq!(act1.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "d");

    // key 2: 'd' -> replaces to 'đ'
    let act2 = ctx.process_key(b'd' as u32, "d", false, Some("d"), 1, 1);
    assert_eq!(act2.action_type, ACTION_REPLACE);
    assert_eq!(ctx.last_composed, "đ");

    // key 3: 'u' arrives before app processed replace, app still reports "d" at cursor 1
    let act3 = ctx.process_key(b'u' as u32, "u", false, Some("d"), 1, 1);
    assert_eq!(act3.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "đu");

    // key 4: 'o' -> app caught up to "đu" at cursor 2
    let act4 = ctx.process_key(b'o' as u32, "o", false, Some("đu"), 2, 2);
    assert_eq!(act4.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "đuo");

    // key 5: 'w' -> replaces to "đươ"
    let act5 = ctx.process_key(b'w' as u32, "w", false, Some("đuo"), 3, 3);
    assert_eq!(act5.action_type, ACTION_REPLACE);
    assert_eq!(ctx.last_composed, "đươ");

    // key 6: 'c' arrives before app processed replace, app still reports "đuo" at cursor 3
    let act6 = ctx.process_key(b'c' as u32, "c", false, Some("đuo"), 3, 3);
    assert_eq!(act6.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "đươc");

    // key 7: 'j' -> replaces to "được"
    let act7 = ctx.process_key(b'j' as u32, "j", false, Some("đươc"), 4, 4);
    assert_eq!(act7.action_type, ACTION_REPLACE);
    assert_eq!(ctx.last_composed, "được");
}

#[test]
fn test_rapid_typing_lagging_surrounding_viet_tone_marks() {
    let mut ctx = ClakContext::new(Method::Telex);
    // Type "việt" (v-i-e-e-t-j) rapidly in a multiline document
    // Surrounding text lags behind at "v" or "vi"
    ctx.process_key(b'v' as u32, "v", false, Some("xin chao "), 9, 9);
    ctx.process_key(b'i' as u32, "i", false, Some("xin chao v"), 10, 10);
    // 1st 'e' -> "vie"
    let act_e1 = ctx.process_key(b'e' as u32, "e", false, Some("xin chao vi"), 11, 11);
    assert_eq!(act_e1.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "vie");

    // 2nd 'e' -> replaces to "viê"
    let act_e2 = ctx.process_key(b'e' as u32, "e", false, Some("xin chao vie"), 12, 12);
    assert_eq!(act_e2.action_type, ACTION_REPLACE);
    assert_eq!(ctx.last_composed, "viê");

    // Client still reports "xin chao vi" at cursor 11 due to Wayland roundtrip latency
    // User immediately types 't' -> "viêt"
    let act_t = ctx.process_key(b't' as u32, "t", false, Some("xin chao vi"), 11, 11);
    assert_eq!(act_t.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "viêt");

    // Client STILL reports "xin chao vi" at cursor 11
    // User types tone 'j' -> "việt"
    // IME MUST NOT RESET or eat letters!
    let act_j = ctx.process_key(b'j' as u32, "j", false, Some("xin chao vi"), 11, 11);
    assert_eq!(act_j.action_type, ACTION_REPLACE);
    assert_eq!(ctx.last_composed, "việt");
}

#[test]
fn test_twitter_draftjs_uppercase_dd_to_capital_d_stroke() {
    let mut ctx = ClakContext::new(Method::Telex);
    // Key 1: 'D' forwarded, editor gets "D\n"
    let act1 = ctx.process_key(b'D' as u32, "D", false, Some("\n"), 0, 0);
    assert_eq!(act1.action_type, ACTION_FORWARD);
    assert_eq!(ctx.last_composed, "D");

    // Twitter / DraftJS remounts DOM, causing reset of IME context
    ctx.reset();
    assert!(ctx.last_composed.is_empty());

    // Key 2: 'D' pressed. Surrounding text reports "D\n" cursor 1.
    // Engine must seed "D", accept uppercase 'D' as telex modifier, and replace to "Đ"!
    let act2 = ctx.process_key(b'D' as u32, "D", false, Some("D\n"), 1, 1);
    assert_eq!(act2.action_type, ACTION_REPLACE);
    assert_eq!(ctx.last_composed, "Đ");
}

#[test]
fn test_delayed_circumflex_and_auto_restore() {
    // detej produces dệt
    assert_eq!(simulate_typing("detej"), "dệt");
    assert_eq!(simulate_typing_fallback("detej"), "dệt");

    // detec auto-restores to detec, detect produces detect
    assert_eq!(simulate_typing("detect"), "detect");
    assert_eq!(simulate_typing_fallback("detect"), "detect");

    // detee cleanly unmerges to dete without triple e
    assert_eq!(simulate_typing("detee"), "dete");
    assert_eq!(simulate_typing_fallback("detee"), "dete");

    // space after incomplete stop coda restores to dete 
    assert_eq!(simulate_typing("dete "), "dete ");
    assert_eq!(simulate_typing_fallback("dete "), "dete ");

    // enter after incomplete stop coda restores to dete\n
    assert_eq!(simulate_typing("dete\n"), "dete\n");
    assert_eq!(simulate_typing_fallback("dete\n"), "dete\n");

    // completed stop codas keep their vietnamese form on space
    assert_eq!(simulate_typing("detej "), "dệt ");
    assert_eq!(simulate_typing_fallback("detej "), "dệt ");

    assert_eq!(simulate_typing("detes "), "dết ");
    assert_eq!(simulate_typing_fallback("detes "), "dết ");

    // non-stop codas keep valid vietnamese words on space
    assert_eq!(simulate_typing("tooi "), "tôi ");
    assert_eq!(simulate_typing_fallback("tooi "), "tôi ");

    assert_eq!(simulate_typing("ddi "), "đi ");
    assert_eq!(simulate_typing_fallback("ddi "), "đi ");

    assert_eq!(simulate_typing("chata "), "chata ");
    assert_eq!(simulate_typing_fallback("chata "), "chata ");

    assert_eq!(simulate_typing("chatas "), "chất ");
    assert_eq!(simulate_typing_fallback("chatas "), "chất ");
}
