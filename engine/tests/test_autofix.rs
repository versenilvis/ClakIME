use clak_engine::autofix::autofix_text;
use clak_engine::Method;

#[test]
fn test_autofix_pure_raw_telex() {
    let input = "ddaay laf moojt baif toans khos";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(changed);
    assert_eq!(output, "đây là một bài toán khó");
}

#[test]
fn test_autofix_mixed_accented_and_raw() {
    let input = "đây laf một bafi toans khó";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(changed);
    assert_eq!(output, "đây là một bài toán khó");
}

#[test]
fn test_autofix_with_punctuations_and_newlines() {
    let input = "ddaay, laf moojt (baif toans) khos!\nchieefu nay veef.";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(changed);
    assert_eq!(output, "đây, là một (bài toán) khó!\nchiều nay về.");
}

#[test]
fn test_autofix_preserves_common_english_words() {
    let input = "this is a test for as of on at in";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(!changed);
    assert_eq!(output, input);
}

#[test]
fn test_autofix_mixed_english_and_vietnamese() {
    let input = "this is ddaay laf a test";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(changed);
    assert_eq!(output, "this is đây là a test");
}

#[test]
fn test_autofix_preserves_urls_and_emails() {
    let input = "xem tai https://example.com/dday hoac lien he admin@dday.vn ddaay laf link";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(changed);
    assert_eq!(
        output,
        "xem tai https://example.com/dday hoac lien he admin@dday.vn đây là link"
    );
}

#[test]
fn test_autofix_preserves_all_caps_and_code_identifiers() {
    let input = "const DDAY_COUNT = 10; let my_var = ddaay;";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(changed);
    assert_eq!(output, "const DDAY_COUNT = 10; let my_var = đây;");
}

#[test]
fn test_autofix_preserves_capitalization() {
    let input = "Ddaay laf Moojt Baif toans";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(changed);
    assert_eq!(output, "Đây là Một Bài toán");
}

#[test]
fn test_autofix_already_correct_vietnamese() {
    let input = "Hôm nay thời tiết thật là đẹp và mát mẻ.";
    let (output, changed) = autofix_text(input, Method::Telex, true);
    assert!(!changed);
    assert_eq!(output, input);
}

#[test]
fn test_autofix_vni_method() {
    let input = "d9a6y la2 mo65t ba2i toa1n kho1";
    let (output, changed) = autofix_text(input, Method::Vni, true);
    assert!(changed);
    assert_eq!(output, "đây là một bài toán khó");
}
