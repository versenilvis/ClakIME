use clak_engine::autocorrect::try_autocorrect_telex;
use clak_engine::learned_typos::{clear_for_test, observe_retype};
use clak_engine::{ClakContext, Method, ACTION_AUTOCORRECT, ACTION_FORWARD, ACTION_COMMIT, ACTION_REPLACE, ACTION_REPLACE_SURROUNDING, ACTION_ADDRESS_BAR_FIX};
use std::ffi::CStr;

fn simulate_typing_autocorrect(input: &str) -> String {
    let mut ctx = ClakContext::new(Method::Telex);
    ctx.autocorrect = true;
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
                | ACTION_ADDRESS_BAR_FIX
                | ACTION_AUTOCORRECT => {
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

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_learned_typos_lifecycle_and_threshold() {
    let _guard = TEST_LOCK.lock().unwrap();
    clear_for_test();

    // candidate typo: "duowcj" for "được" (not in english, similar, target is valid vietnamese)
    // 1st time observed: counter = 1, not yet in active learned typos
    let res1 = observe_retype("duowcj", "được");
    assert_eq!(res1, None);
    assert_eq!(try_autocorrect_telex("duowcj", true, false), None);

    // 2nd time observed: counter = 2 >= threshold, successfully learned!
    let res2 = observe_retype("duowcj", "được");
    assert_eq!(res2, Some("được".to_string()));

    // now autocorrect recognizes it immediately
    assert_eq!(
        try_autocorrect_telex("duowcj", true, false),
        Some("được".to_string())
    );
    // casing is preserved
    assert_eq!(
        try_autocorrect_telex("Duowcj", true, false),
        Some("Được".to_string())
    );
}

#[test]
fn test_learned_typos_guards_english_never_learned() {
    let _guard = TEST_LOCK.lock().unwrap();
    clear_for_test();

    // user types english word "class" and corrects to "const" (both english)
    // must be rejected immediately to avoid polluting code/english typing
    let res = observe_retype("class", "const");
    assert_eq!(res, None);
    assert_eq!(try_autocorrect_telex("class", true, false), None);
}

#[test]
fn test_learned_typos_guards_invalid_target_never_learned() {
    let _guard = TEST_LOCK.lock().unwrap();
    clear_for_test();

    // user types "asdf" and retypes "qwer" (nonsense / not vietnamese dictionary)
    let res = observe_retype("asdf", "qwer");
    assert_eq!(res, None);
    assert_eq!(try_autocorrect_telex("asdf", true, false), None);
}

#[test]
fn test_learned_typos_guards_dissimilar_words_never_learned() {
    let _guard = TEST_LOCK.lock().unwrap();
    clear_for_test();

    // user completely changes mind: types "xanh" (green) then deletes to type "đỏ" (red)
    // completely unrelated words (distance > 3) -> rejected
    let res = observe_retype("xanh", "đỏ");
    assert_eq!(res, None);
    assert_eq!(try_autocorrect_telex("xanh", true, false), None);
}

#[test]
fn test_learned_typos_e2e_ime_typing_and_backspace_retype() {
    let _guard = TEST_LOCK.lock().unwrap();
    clear_for_test();

    // directly teach a custom typo "khongn" -> "không" twice
    observe_retype("khongn", "không");
    observe_retype("khongn", "không");

    // now simulate typing in IME with autocorrect enabled
    let typed = simulate_typing_autocorrect("khongn ");
    assert_eq!(typed, "không ");
}
