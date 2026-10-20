use clak_engine::autocorrect::try_autocorrect_telex;

#[test]
fn test_scrambled_vowel_transposition_nheiuef_to_nhieu() {
    let res = try_autocorrect_telex("nheiuef", true, false);
    assert_eq!(res, Some("nhiều".to_string()));
}

#[test]
fn test_scrambled_coda_inversion_thoogn_to_thong() {
    let res = try_autocorrect_telex("thoogn", true, false);
    assert_eq!(res, Some("thông".to_string()));
}

#[test]
fn test_scrambled_double_swap_beeits_to_biet() {
    let res = try_autocorrect_telex("beeits", true, false);
    assert_eq!(res, Some("biết".to_string()));
}

#[test]
fn test_scrambled_coda_before_vowel_bnja_to_ban() {
    let res = try_autocorrect_telex("bnja", true, false);
    assert_eq!(res, Some("bạn".to_string()));
}

#[test]
fn test_scrambled_coda_before_tone_bajn_engine_native() {
    // clak engine converts bajn to bạn natively
    assert_eq!(
        clak_engine::engine::convert_telex("bajn", true, false),
        "bạn"
    );
    assert_eq!(try_autocorrect_telex("bajn", true, false), None);
}

#[test]
fn test_scrambled_famous_khnog_to_khong() {
    let res = try_autocorrect_telex("khnog", true, false);
    assert_eq!(res, Some("không".to_string()));
    let res_cap = try_autocorrect_telex("Khnog", true, false);
    assert_eq!(res_cap, Some("Không".to_string()));
}

#[test]
fn test_scrambled_double_o_khoogn_and_khnoog_to_khong() {
    assert_eq!(
        try_autocorrect_telex("khoogn", true, false),
        Some("không".to_string())
    );
    assert_eq!(
        try_autocorrect_telex("khnoog", true, false),
        Some("không".to_string())
    );
    // khonog is handled natively via engine roof crossing
    assert_eq!(
        clak_engine::engine::convert_telex("khonog", true, false),
        "không"
    );
    assert_eq!(try_autocorrect_telex("khonog", true, false), None);
}

#[test]
fn test_scrambled_triphthong_kieur_and_nhieuf() {
    assert_eq!(
        try_autocorrect_telex("kieur", true, false),
        Some("kiểu".to_string())
    );
    assert_eq!(
        try_autocorrect_telex("nhieuf", true, false),
        Some("nhiều".to_string())
    );
    assert_eq!(
        try_autocorrect_telex("hieur", true, false),
        Some("hiểu".to_string())
    );
    assert_eq!(
        try_autocorrect_telex("yeur", true, false),
        Some("yểu".to_string())
    );
}

#[test]
fn test_scrambled_dudowjc_to_duoc() {
    assert_eq!(
        try_autocorrect_telex("dudowjc", true, false),
        Some("được".to_string())
    );
    assert_eq!(
        try_autocorrect_telex("Dudowjc", true, false),
        Some("Được".to_string())
    );
    assert_eq!(
        try_autocorrect_telex("dduowjc", true, false),
        Some("được".to_string())
    );
    assert_eq!(
        try_autocorrect_telex("Dduowjc", true, false),
        Some("Được".to_string())
    );
}

#[test]
fn test_scrambled_nguwoif_engine_native() {
    // answering the user's question: "nguwoif (thiếu một w), mình chưa biết engine có ra người không"
    // YES: clak engine horn propagation automatically handles nguwoif natively!
    let native = clak_engine::engine::convert_telex("nguwoif", true, false);
    assert_eq!(native, "người");
    // because convert_telex already yields valid vietnamese "người", autocorrect safely returns None
    assert_eq!(try_autocorrect_telex("nguwoif", true, false), None);
}

#[test]
fn test_ambiguous_two_candidates_returns_none() {
    // when a scrambled word has 2 or more dictionary candidates, autocorrect must NOT guess
    // it must return None to remain strictly safe
    // e.g. "caans" could be "cán" or "cắn" or "cấn" -> ambiguous -> None
    assert_eq!(try_autocorrect_telex("caans", true, false), None);
}

#[test]
fn test_scrambled_casing_preserved() {
    let res = try_autocorrect_telex("Nheiuef", true, false);
    assert_eq!(res, Some("Nhiều".to_string()));
}

#[test]
fn test_gn_and_hn_english_words_never_autocorrected() {
    // coda inversion gn->ng and hn->nh must never trigger on valid english words
    assert_eq!(try_autocorrect_telex("sign", true, false), None);
    assert_eq!(try_autocorrect_telex("design", true, false), None);
    assert_eq!(try_autocorrect_telex("align", true, false), None);
    assert_eq!(try_autocorrect_telex("john", true, false), None);
    assert_eq!(try_autocorrect_telex("realign", true, false), None);
    assert_eq!(try_autocorrect_telex("assign", true, false), None);
    assert_eq!(try_autocorrect_telex("campaign", true, false), None);
    assert_eq!(try_autocorrect_telex("foreign", true, false), None);
    assert_eq!(try_autocorrect_telex("sovereign", true, false), None);
}

#[test]
fn test_tech_and_system_words_never_autocorrected() {
    assert_eq!(try_autocorrect_telex("uinput", true, false), None);
    assert_eq!(try_autocorrect_telex("fcitx", true, false), None);
    assert_eq!(try_autocorrect_telex("fcitx5", true, false), None);
    assert_eq!(try_autocorrect_telex("wayland", true, false), None);
    assert_eq!(try_autocorrect_telex("hyprland", true, false), None);
    assert_eq!(try_autocorrect_telex("cmake", true, false), None);
    assert_eq!(try_autocorrect_telex("cargo", true, false), None);
    assert_eq!(try_autocorrect_telex("rustc", true, false), None);
    assert_eq!(try_autocorrect_telex("draftjs", true, false), None);
}

#[test]
fn test_english_words_never_autocorrected() {
    // words ending in f
    assert_eq!(try_autocorrect_telex("brief", true, false), None);
    assert_eq!(try_autocorrect_telex("chief", true, false), None);
    assert_eq!(try_autocorrect_telex("staff", true, false), None);
    assert_eq!(try_autocorrect_telex("cliff", true, false), None);
    assert_eq!(try_autocorrect_telex("shelf", true, false), None);

    // words ending in s, r, x, w
    assert_eq!(try_autocorrect_telex("server", true, false), None);
    assert_eq!(try_autocorrect_telex("user", true, false), None);
    assert_eq!(try_autocorrect_telex("test", true, false), None);
    assert_eq!(try_autocorrect_telex("tests", true, false), None);
    assert_eq!(try_autocorrect_telex("view", true, false), None);
    assert_eq!(try_autocorrect_telex("review", true, false), None);
    assert_eq!(try_autocorrect_telex("complex", true, false), None);
    assert_eq!(try_autocorrect_telex("syntax", true, false), None);

    // words with double vowels
    assert_eq!(try_autocorrect_telex("green", true, false), None);
    assert_eq!(try_autocorrect_telex("screen", true, false), None);
    assert_eq!(try_autocorrect_telex("speed", true, false), None);
    assert_eq!(try_autocorrect_telex("food", true, false), None);
    assert_eq!(try_autocorrect_telex("good", true, false), None);
}

#[test]
fn test_already_valid_vietnamese_words_never_modified() {
    assert_eq!(try_autocorrect_telex("người", true, false), None);
    assert_eq!(try_autocorrect_telex("nhiều", true, false), None);
    assert_eq!(try_autocorrect_telex("không", true, false), None);
    assert_eq!(try_autocorrect_telex("được", true, false), None);
    assert_eq!(try_autocorrect_telex("biết", true, false), None);
    assert_eq!(try_autocorrect_telex("mình", true, false), None);
    assert_eq!(try_autocorrect_telex("chuyển", true, false), None);
}

#[test]
fn test_code_and_acronyms_never_autocorrected() {
    assert_eq!(try_autocorrect_telex("HTML", true, false), None);
    assert_eq!(try_autocorrect_telex("API", true, false), None);
    assert_eq!(try_autocorrect_telex("iPhone", true, false), None);
    assert_eq!(try_autocorrect_telex("camelCase", true, false), None);
}

#[test]
fn test_foreign_consonant_clusters_never_autocorrected() {
    assert_eq!(try_autocorrect_telex("thre", true, false), None);
    assert_eq!(try_autocorrect_telex("thread", true, false), None);
    assert_eq!(try_autocorrect_telex("threads", true, false), None);
    assert_eq!(try_autocorrect_telex("three", true, false), None);
    assert_eq!(try_autocorrect_telex("thro", true, false), None);
    assert_eq!(try_autocorrect_telex("pro", true, false), None);
    assert_eq!(try_autocorrect_telex("pre", true, false), None);
    assert_eq!(try_autocorrect_telex("bro", true, false), None);
    assert_eq!(try_autocorrect_telex("cro", true, false), None);
    assert_eq!(try_autocorrect_telex("drop", true, false), None);
    assert_eq!(try_autocorrect_telex("group", true, false), None);
    assert_eq!(try_autocorrect_telex("slow", true, false), None);
    assert_eq!(try_autocorrect_telex("class", true, false), None);
}

#[test]
fn test_benchmark_on_repo_identifiers_zero_false_positives() {
    let repo_words = include_str!("repo_identifiers.txt");
    let mut false_positives = Vec::new();

    for line in repo_words.lines() {
        let w = line.trim();
        if w.len() < 3 || w.len() > 10 {
            continue;
        }
        // if this identifier is pure alphabetic
        if w.chars().all(|c| c.is_ascii_alphabetic()) {
            if let Some(changed) = try_autocorrect_telex(w, true, false) {
                // report false positive
                false_positives.push((w.to_string(), changed));
            }
        }
    }

    if !false_positives.is_empty() {
        eprintln!("Detected false positives ({}):", false_positives.len());
        for (orig, target) in &false_positives {
            eprintln!("  {:?} -> {:?}", orig, target);
        }
    }

    assert_eq!(
        false_positives.len(),
        0,
        "Autocorrect must have 0 false positives on repo identifiers"
    );
}
