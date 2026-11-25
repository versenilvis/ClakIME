use crate::charset;
use crate::engine;
use crate::spelling;
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Telex,
    Vni,
    TeipVni,
    Viqr,
}

static DICT: OnceLock<Vec<&'static str>> = OnceLock::new();
static BASES: OnceLock<Vec<String>> = OnceLock::new();

pub fn dict() -> &'static Vec<&'static str> {
    DICT.get_or_init(|| include_str!("../data/vietnamese.cm.dict").lines().collect())
}

pub fn bases() -> &'static Vec<String> {
    BASES.get_or_init(|| {
        let mut v: Vec<String> = dict().iter().map(|w| charset::remove_tone(w)).collect();
        v.sort();
        v.dedup();
        v
    })
}

pub struct ClakCore {
    method: Method,
    modern: bool,
    short_w: bool,
    auto_restore: bool,
    auto_restore_english: bool,
    dict: bool,
    allow_foreign_consonants: bool,
    user_words: HashSet<String>,
    user_bases: Vec<String>,
}

pub type ClakEngine = ClakCore;

pub fn is_open_short_vowel_with_onset(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 2 {
        return false;
    }
    let last = chars[chars.len() - 1];
    let is_short_vowel = matches!(
        last,
        'ă' | 'ằ'
            | 'ắ'
            | 'ẳ'
            | 'ẵ'
            | 'ặ'
            | 'â'
            | 'ầ'
            | 'ấ'
            | 'ẩ'
            | 'ẫ'
            | 'ậ'
            | 'Ă'
            | 'Ằ'
            | 'Ắ'
            | 'Ẳ'
            | 'Ẵ'
            | 'Ặ'
            | 'Â'
            | 'Ầ'
            | 'Ấ'
            | 'Ẩ'
            | 'Ẫ'
            | 'Ậ'
    );
    if !is_short_vowel {
        return false;
    }
    let prev = chars[chars.len() - 2];
    !crate::spelling::is_vowel(prev)
}

impl ClakCore {
    pub fn new(method: Method) -> Self {
        ClakCore {
            method,
            modern: true,
            short_w: false,
            auto_restore: false,
            auto_restore_english: false,
            dict: false,
            allow_foreign_consonants: false,
            user_words: HashSet::new(),
            user_bases: Vec::new(),
        }
    }

    pub fn set_method(&mut self, method: Method) {
        self.method = method;
    }
    pub fn method(&self) -> Method {
        self.method
    }
    pub fn set_modern(&mut self, v: bool) {
        self.modern = v;
    }
    pub fn modern(&self) -> bool {
        self.modern
    }
    pub fn set_short_w(&mut self, v: bool) {
        self.short_w = v;
    }
    pub fn short_w(&self) -> bool {
        self.short_w
    }
    pub fn set_auto_restore(&mut self, v: bool) {
        self.auto_restore = v;
    }
    pub fn auto_restore(&self) -> bool {
        self.auto_restore
    }
    pub fn set_auto_restore_english(&mut self, v: bool) {
        self.auto_restore_english = v;
    }
    pub fn auto_restore_english(&self) -> bool {
        self.auto_restore_english
    }
    pub fn set_dict(&mut self, v: bool) {
        self.dict = v;
    }
    pub fn set_allow_foreign_consonants(&mut self, v: bool) {
        self.allow_foreign_consonants = v;
    }
    pub fn allow_foreign_consonants(&self) -> bool {
        self.allow_foreign_consonants
    }

    pub fn add_word(&mut self, word: &str) {
        let w = word.to_lowercase();
        let base = charset::remove_tone(&w);
        self.user_words.insert(w);
        if let Err(pos) = self.user_bases.binary_search(&base) {
            self.user_bases.insert(pos, base);
        }
    }

    pub fn clear_words(&mut self) {
        self.user_words.clear();
        self.user_bases.clear();
    }

    pub fn is_exact_dict_word(&self, word: &str) -> bool {
        let w = word
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .to_lowercase();
        self.user_words.contains(&w) || dict().binary_search(&w.as_str()).is_ok()
    }

    fn dict_known(&self, word: &str) -> bool {
        let word = word.trim_start_matches(|c: char| c.is_ascii_digit());
        if word.is_empty() {
            return false;
        }
        let base = charset::remove_tone(word);
        if base != word {
            if self.user_words.contains(word) || dict().binary_search(&word).is_ok() {
                return true;
            }
            let user_match = match self.user_bases.binary_search(&base) {
                Ok(i) => i + 1 < self.user_bases.len() && self.user_bases[i + 1].starts_with(&base),
                Err(i) => i < self.user_bases.len() && self.user_bases[i].starts_with(&base),
            };
            if user_match {
                return true;
            }
            let bs = bases();
            match bs.binary_search(&base) {
                Ok(i) => i + 1 < bs.len() && bs[i + 1].starts_with(&base),
                Err(i) => i < bs.len() && bs[i].starts_with(&base),
            }
        } else {
            let user_match = match self.user_bases.binary_search(&base) {
                Ok(_) => true,
                Err(i) => i < self.user_bases.len() && self.user_bases[i].starts_with(&base),
            };
            if user_match {
                return true;
            }
            let bs = bases();
            match bs.binary_search(&base) {
                Ok(_) => true,
                Err(i) => i < bs.len() && bs[i].starts_with(&base),
            }
        }
    }

    pub fn transform(&self, input: &str) -> String {
        let composed = match self.method {
            Method::Telex => engine::convert_telex_with_options(
                input,
                self.modern,
                self.short_w,
                self.allow_foreign_consonants,
            ),
            Method::Vni => engine::convert_vni_with_options(
                input,
                self.modern,
                self.short_w,
                self.allow_foreign_consonants,
            ),
            Method::TeipVni => engine::convert_teip_vni_with_options(
                input,
                self.modern,
                self.short_w,
                self.allow_foreign_consonants,
            ),
            Method::Viqr => engine::convert_viqr(input),
        };
        let is_en = crate::english_dict::is_english(input);
        let is_foreign_initial =
            self.allow_foreign_consonants && starts_with_foreign_initial(&composed);
        let known = if self.dict {
            if is_en && !is_foreign_initial {
                if self.auto_restore_english {
                    self.is_exact_dict_word(&composed)
                } else {
                    self.is_exact_dict_word(&composed) || is_open_short_vowel_with_onset(&composed)
                }
            } else if is_foreign_initial {
                spelling::is_valid_cvc_with_options(&composed, true)
            } else {
                self.dict_known(&composed.to_lowercase())
                    || is_informal_or_loanword(&composed)
                    || is_open_short_vowel_with_onset(&composed)
            }
        } else {
            spelling::is_valid_cvc_with_options(&composed, self.allow_foreign_consonants)
        };
        if self.auto_restore
            && should_auto_restore(input, &composed, known, self.auto_restore_english)
        {
            input.to_string()
        } else {
            composed
        }
    }

    pub fn is_valid(s: &str) -> bool {
        spelling::is_valid_cvc(s)
    }
}

pub fn starts_with_foreign_initial(s: &str) -> bool {
    let s = s.trim_start_matches(|c: char| c.is_ascii_digit());
    s.chars()
        .next()
        .is_some_and(|c| matches!(c.to_ascii_lowercase(), 'z' | 'f' | 'w' | 'j'))
}

pub(crate) fn is_informal_or_loanword(s: &str) -> bool {
    let lower: String = s
        .chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();
    if lower == "ôkê" || lower == "oke" {
        return true;
    }
    // chat abbreviations starting with k
    if lower.starts_with('k') && spelling::is_valid_cvc_with_options(&lower, true) {
        return true;
    }
    // abbreviations starting with dc or vc
    if (lower.starts_with("đc") || lower.starts_with("vc"))
        && (lower.len() == 2 || spelling::is_valid_cvc_with_options(&lower, true))
    {
        return true;
    }
    // loanword initials
    if lower.len() >= 2 {
        let mut chars = lower.chars();
        let c0 = chars.next().unwrap_or('\0');
        let c1 = chars.next().unwrap_or('\0');
        if spelling::is_loanword_onset_allowed(c0, c1, &lower, false)
            && spelling::is_valid_cvc_with_options(&lower, true)
        {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
pub(crate) fn count_vowel_clusters(s: &str) -> usize {
    let mut count = 0;
    let mut in_vowel = false;
    for c in s.chars() {
        let is_v = matches!(
            c.to_ascii_lowercase(),
            'a' | 'e'
                | 'i'
                | 'o'
                | 'u'
                | 'y'
                | 'à'
                | 'á'
                | 'ả'
                | 'ã'
                | 'ạ'
                | 'â'
                | 'ầ'
                | 'ấ'
                | 'ẩ'
                | 'ẫ'
                | 'ậ'
                | 'ă'
                | 'ằ'
                | 'ắ'
                | 'ẳ'
                | 'ẵ'
                | 'ặ'
                | 'è'
                | 'é'
                | 'ẻ'
                | 'ẽ'
                | 'ẹ'
                | 'ê'
                | 'ề'
                | 'ế'
                | 'ể'
                | 'ễ'
                | 'ệ'
                | 'ì'
                | 'í'
                | 'ỉ'
                | 'ĩ'
                | 'ị'
                | 'ò'
                | 'ó'
                | 'ỏ'
                | 'õ'
                | 'ọ'
                | 'ô'
                | 'ồ'
                | 'ố'
                | 'ổ'
                | 'ỗ'
                | 'ộ'
                | 'ơ'
                | 'ờ'
                | 'ớ'
                | 'ở'
                | 'ỡ'
                | 'ợ'
                | 'ù'
                | 'ú'
                | 'ủ'
                | 'ũ'
                | 'ụ'
                | 'ư'
                | 'ừ'
                | 'ứ'
                | 'ử'
                | 'ữ'
                | 'ự'
                | 'ỳ'
                | 'ý'
                | 'ỷ'
                | 'ỹ'
                | 'ỵ'
        );
        if is_v {
            if !in_vowel {
                count += 1;
                in_vowel = true;
            }
        } else {
            in_vowel = false;
        }
    }
    count
}

pub(crate) fn ends_with_doubled_tone_modifier(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.ends_with("ss")
        || lower.ends_with("ff")
        || lower.ends_with("rr")
        || lower.ends_with("xx")
        || lower.ends_with("jj")
}

pub(crate) fn should_auto_restore(
    input: &str,
    composed: &str,
    known_vn: bool,
    auto_restore_english: bool,
) -> bool {
    if composed == input {
        return false;
    }
    if has_vn_markers(input) {
        return false;
    }

    if known_vn {
        return false;
    }

    let is_en = crate::english_dict::is_english(input);

    // skip all-uppercase acronyms with vietnamese characters unless english word
    let is_all_upper = input.len() >= 2
        && input
            .chars()
            .all(|c| !c.is_alphabetic() || c.is_uppercase())
        && input.chars().any(|c| c.is_uppercase());
    if is_all_upper && !is_en {
        return false;
    }

    // trailing doubled tone modifiers that actually swallowed one of the modifier characters are intentional un-toning
    if (!is_en || !auto_restore_english) && ends_with_doubled_tone_modifier(input) {
        let lower_in = input.to_ascii_lowercase();
        let lower_comp = composed.to_ascii_lowercase();
        let mod_suffix = &lower_in[lower_in.len() - 2..];
        if !lower_comp.ends_with(mod_suffix) && input.len() > composed.len() {
            return false;
        }
    }

    // un-toning resolved to a valid english word while raw is not valid english
    if crate::english_dict::is_english(composed) && !is_en {
        return false;
    }

    let swallowed_char =
        (!has_consecutive_modifiers(input) || is_en) && input.len() > composed.len();

    let all_ascii = composed.is_ascii();

    if (!all_ascii || swallowed_char || is_en) && !known_vn && !has_vn_markers(input) {
        return true;
    }

    false
}

#[allow(dead_code)]
pub(crate) fn has_consecutive_modifiers(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    lower.contains("ss")
        || lower.contains("ff")
        || lower.contains("rr")
        || lower.contains("xx")
        || lower.contains("jj")
        || lower.contains("ww")
        || lower.contains("aa")
        || lower.contains("ee")
        || lower.contains("oo")
        || lower.contains("dd")
}

pub(crate) fn has_vn_markers(input: &str) -> bool {
    let lower = input.to_lowercase();
    let dd_marker = lower.starts_with("dd")
        || (lower.starts_with("dud") && lower.contains('o'))
        || lower.match_indices("dd").any(|(i, _)| {
            i > 0
                && !matches!(
                    lower[..i].chars().next_back(),
                    Some('a' | 'e' | 'i' | 'o' | 'u' | 'y')
                )
        });
    dd_marker
        || lower.contains("aaa")
        || lower.contains("eee")
        || lower.contains("ooo")
        || lower.contains("aww")
        || lower.contains("oww")
        || lower.contains("uww")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eng(auto_restore: bool, dict: bool) -> ClakCore {
        let mut e = ClakCore::new(Method::Telex);
        e.set_auto_restore(auto_restore);
        e.set_dict(dict);
        e
    }

    #[test]
    fn dict_restores_nonwords() {
        let e = eng(true, true);
        assert_eq!(e.transform("luwocs"), "luwocs");
        assert_eq!(e.transform("need"), "need");
        assert_eq!(e.transform("force"), "force");
    }

    #[test]
    fn dict_keeps_prefixes_realtime() {
        let e = eng(true, true);
        assert_eq!(e.transform("phuowc"), "phươc");
        assert_eq!(e.transform("phuowcs"), "phước");
        assert_eq!(e.transform("truwoc"), "trươc");
        assert_eq!(e.transform("luow"), "lươ");
        assert_eq!(e.transform("luowj"), "lượ");
        assert_eq!(e.transform("luowjc"), "lược");

        let mut e2 = eng(true, true);
        e2.add_word("lước");
        assert_eq!(e2.transform("luwoc"), "lươc");
    }

    #[test]
    fn dict_keeps_real_words() {
        let e = eng(true, true);
        assert_eq!(e.transform("truwocs"), "trước");
        assert_eq!(e.transform("khoong"), "không");
        assert_eq!(e.transform("Vieetj"), "Việt");
        assert_eq!(e.transform("luoojcw"), "lược");
        assert_eq!(e.transform("ginf"), "gìn");
        assert_eq!(e.transform("gifn"), "gìn");

        let e_rule = eng(true, false);
        assert_eq!(e_rule.transform("ginf"), "gìn");
        assert_eq!(e_rule.transform("gifn"), "gìn");
    }

    #[test]
    fn dict_keeps_abbreviations() {
        let e = eng(true, true);
        assert_eq!(e.transform("ddc"), "đc");
    }

    #[test]
    fn auto_restore_unikey_style_english_word() {
        let e = eng(true, false);
        assert_eq!(e.transform("dow"), "dơ");
        assert_eq!(e.transform("doww"), "dow");
        assert_eq!(e.transform("dowwnlo"), "downlo");
        assert_eq!(e.transform("dowwnload"), "download");
        assert_eq!(e.transform("dowwnloads"), "downloads");

        let e = eng(true, true);
        assert_eq!(e.transform("dowwnload"), "download");
    }

    #[test]
    fn test_doubled_modifier_english_words_and_un_toning() {
        // default mode (auto_restore_english = false): doubled trailing modifier strips tone
        let e = eng(true, false);
        assert_eq!(e.transform("mis"), "mí");
        assert_eq!(e.transform("miss"), "mis");
        assert_eq!(e.transform("misst"), "mist");
        assert_eq!(e.transform("misss"), "miss");
        assert_eq!(e.transform("missst"), "misst");
        assert_eq!(e.transform("boss"), "bos");
        assert_eq!(e.transform("bosss"), "boss");
        assert_eq!(e.transform("pass"), "pas");
        assert_eq!(e.transform("passt"), "past");
        assert_eq!(e.transform("passs"), "pass");
        assert_eq!(e.transform("class"), "class");
        assert_eq!(e.transform("less"), "les");
        assert_eq!(e.transform("lesst"), "lest");
        assert_eq!(e.transform("lesss"), "less");
        assert_eq!(e.transform("mess"), "mes");
        assert_eq!(e.transform("messs"), "mess");
        assert_eq!(e.transform("err"), "er");
        assert_eq!(e.transform("errr"), "err");
        assert_eq!(e.transform("coss"), "cos");
        assert_eq!(e.transform("toanss"), "toans");
        assert_eq!(e.transform("nhass"), "nhas");

        // when auto_restore_english is enabled: doubled trailing modifier restores english word directly
        let mut e_en = eng(true, false);
        e_en.set_auto_restore_english(true);
        assert_eq!(e_en.transform("miss"), "miss");
        assert_eq!(e_en.transform("boss"), "boss");
        assert_eq!(e_en.transform("bosss"), "boss");
        assert_eq!(e_en.transform("pass"), "pass");
        assert_eq!(e_en.transform("passs"), "pass");
        assert_eq!(e_en.transform("class"), "class");
        assert_eq!(e_en.transform("less"), "less");
        assert_eq!(e_en.transform("lesss"), "less");
        assert_eq!(e_en.transform("mess"), "mess");
        assert_eq!(e_en.transform("messs"), "mess");
        assert_eq!(e_en.transform("err"), "err");
        assert_eq!(e_en.transform("errr"), "err");
        assert_eq!(e_en.transform("coss"), "cos");

        // intervocalic doubled modifiers in english words
        assert_eq!(e.transform("showoff"), "showoff");
        assert_eq!(e.transform("error"), "error");
        assert_eq!(e.transform("sorry"), "sorry");
        assert_eq!(e.transform("tomorrow"), "tomorrow");
        assert_eq!(e.transform("deepseek"), "deepseek");
        assert_eq!(e.transform("DeepSeek"), "DeepSeek");
        assert_eq!(e.transform("messenger"), "messenger");
        assert_eq!(e.transform("different"), "different");
        assert_eq!(e.transform("difficult"), "difficult");
        assert_eq!(e.transform("traffic"), "traffic");
        assert_eq!(e.transform("buffer"), "buffer");
        assert_eq!(e.transform("session"), "session");
        assert_eq!(e.transform("lesson"), "lesson");
        assert_eq!(e.transform("coffee"), "coffee");
        assert_eq!(e.transform("office"), "office");
        assert_eq!(e.transform("enger"), "enger");

        // un-toning telex cleanup
        assert_eq!(e.transform("mixx"), "mix");
        assert_eq!(e.transform("mixxer"), "mixer");
        assert_eq!(e.transform("fixx"), "fix");
        assert_eq!(e.transform("beffore"), "before");
        assert_eq!(e.transform("resset"), "reset");
        assert_eq!(e.transform("herro"), "hero");

        // contraction stems and tech identifiers
        assert_eq!(e.transform("doesn"), "doesn");
        assert_eq!(e.transform("doesnt"), "doesnt");
        assert_eq!(e.transform("didn"), "didn");
        assert_eq!(e.transform("didnt"), "didnt");
        assert_eq!(e.transform("wasn"), "wasn");
        assert_eq!(e.transform("wasnt"), "wasnt");
        assert_eq!(e.transform("isnt"), "isnt");
        assert_eq!(e.transform("nginx"), "nginx");
        assert_eq!(e.transform("docker"), "docker");

        let e = eng(true, true);
        assert_eq!(e.transform("ins"), "ín");
        assert_eq!(e.transform("miss"), "mis");
        assert_eq!(e.transform("misst"), "mist");
        assert_eq!(e.transform("misss"), "miss");
        assert_eq!(e.transform("boss"), "bos");
        assert_eq!(e.transform("bosss"), "boss");
        assert_eq!(e.transform("mess"), "mes");
        assert_eq!(e.transform("messs"), "mess");
        assert_eq!(e.transform("coss"), "cos");
        assert_eq!(e.transform("error"), "error");
        assert_eq!(e.transform("mixx"), "mix");

        let mut e_dict_en = eng(true, true);
        e_dict_en.set_auto_restore_english(true);
        assert_eq!(e_dict_en.transform("miss"), "miss");
        assert_eq!(e_dict_en.transform("boss"), "boss");
        assert_eq!(e_dict_en.transform("mess"), "mess");
    }

    #[test]
    fn test_foreign_initial_consonants_no_tone() {
        let e = eng(true, false);
        assert_eq!(e.transform("just"), "just");
        assert_eq!(e.transform("jus"), "jus");
        assert_eq!(e.transform("fast"), "fast");

        let e = eng(true, true);
        assert_eq!(e.transform("just"), "just");
        assert_eq!(e.transform("jus"), "jus");
        assert_eq!(e.transform("fast"), "fast");
    }

    #[test]
    fn test_allow_foreign_consonants() {
        let mut e = eng(true, false);
        e.set_allow_foreign_consonants(true);
        assert_eq!(e.transform("zui"), "zui");
        assert_eq!(e.transform("zer"), "zẻ");
        assert_eq!(e.transform("zuis"), "zúi");
        assert_eq!(e.transform("zuir"), "zủi");
        assert_eq!(e.transform("fair"), "fải");
        assert_eq!(e.transform("fee"), "fê");
        assert_eq!(e.transform("was"), "wá");
        assert_eq!(e.transform("jups"), "júp");
        assert_eq!(e.transform("jif"), "jì");

        let mut e_dict = eng(true, true);
        e_dict.set_allow_foreign_consonants(true);
        assert_eq!(e_dict.transform("zui"), "zui");
        assert_eq!(e_dict.transform("zer"), "zẻ");
        assert_eq!(e_dict.transform("zuis"), "zúi");
        assert_eq!(e_dict.transform("fair"), "fải");
        assert_eq!(e_dict.transform("was"), "wá");
        assert_eq!(e_dict.transform("jups"), "júp");
    }

    #[test]
    fn test_invalid_initial_consonant_vowel_pairs_no_tone() {
        let e = eng(true, false);
        assert_eq!(e.transform("ghos"), "ghos");
        assert_eq!(e.transform("ghoss"), "ghoss");
        assert_eq!(e.transform("ghost"), "ghost");
        assert_eq!(e.transform("ghostty"), "ghostty");
        assert_eq!(e.transform("nghos"), "nghos");
        assert_eq!(e.transform("ghes"), "ghé");
        assert_eq!(e.transform("nghes"), "nghé");

        let e = eng(true, true);
        assert_eq!(e.transform("ghos"), "ghos");
        assert_eq!(e.transform("ghoss"), "ghoss");
        assert_eq!(e.transform("ghost"), "ghost");
        assert_eq!(e.transform("ghostty"), "ghostty");
    }

    #[test]
    fn auto_restore_shortw_ww_english_word() {
        let mut e = eng(true, false);
        e.set_short_w(true);
        assert_eq!(e.transform("w"), "ư");
        assert_eq!(e.transform("ww"), "w");
        assert_eq!(e.transform("wwa"), "wa");
        assert_eq!(e.transform("wway"), "way");
        assert_eq!(e.transform("wwayl"), "wayl");
        assert_eq!(e.transform("wwayla"), "wayla");
        assert_eq!(e.transform("wwayland"), "wayland");
        assert_eq!(e.transform("wwindow"), "window");

        // w after digits is a unit, not u-horn
        assert_eq!(e.transform("100w"), "100w");
        assert_eq!(e.transform("100W"), "100W");
        assert_eq!(e.transform("65w"), "65w");
        assert_eq!(e.transform("2w"), "2w");

        // digits starting a vietnamese word
        assert_eq!(e.transform("1casi"), "1cái");
        assert_eq!(e.transform("5trawm"), "5trăm");
        assert_eq!(e.transform("4tuoori"), "4tuổi");

        let mut e = eng(true, true);
        e.set_short_w(true);
        assert_eq!(e.transform("wwayland"), "wayland");
        assert_eq!(e.transform("100w"), "100w");
        assert_eq!(e.transform("1casi"), "1cái");
    }

    #[test]
    fn auto_restore_english_dd_words() {
        let e = eng(true, false);
        assert_eq!(e.transform("add"), "add");
        assert_eq!(e.transform("addr"), "addr");
        assert_eq!(e.transform("addre"), "addre");
        assert_eq!(e.transform("addres"), "addres");
        assert_eq!(e.transform("address"), "address");
        assert_eq!(e.transform("ladder"), "ladder");
        assert_eq!(e.transform("odd"), "odd");

        let e = eng(true, true);
        assert_eq!(e.transform("address"), "address");

        let e = eng(true, false);
        assert_eq!(e.transform("ddc"), "đc");
        assert_eq!(e.transform("ddi"), "đi");
        assert_eq!(e.transform("DDaf"), "Đà");
        assert_eq!(e.transform("vcdd"), "vcđ");
        assert_eq!(e.transform("kdd"), "kđ");
        assert_eq!(e.transform("cmdd"), "cmđ");
    }

    #[test]
    fn double_vowel_swap_survives_auto_restore() {
        let e = eng(true, false);
        assert_eq!(e.transform("lawsma"), "lấm");
        let e = eng(true, true);
        assert_eq!(e.transform("lawsma"), "lấm");
        assert_eq!(e.transform("luowjco"), "luộc");
    }

    #[test]
    fn dict_off_keeps_rule_based_behavior() {
        let e = eng(true, false);
        assert_eq!(e.transform("is"), "í");
        assert_eq!(e.transform("uaj"), "ụa");
        assert_eq!(e.transform("uja"), "ụa");
        assert_eq!(e.transform("luwocs"), "lước");
        assert_eq!(e.transform("need"), "need");
    }

    #[test]
    fn dict_user_words_override() {
        let mut e = eng(true, true);
        assert_eq!(e.transform("luwocs"), "luwocs");
        e.add_word("Lước");
        assert_eq!(e.transform("luwocs"), "lước");
        assert_eq!(e.transform("LUWOCS"), "LƯỚC");
    }

    #[test]
    fn dict_embedded_is_sorted() {
        let dict =
            DICT.get_or_init(|| include_str!("../data/vietnamese.cm.dict").lines().collect());
        assert!(dict.len() > 7000, "dict unexpectedly small: {}", dict.len());
        for w in dict.windows(2) {
            assert!(w[0] < w[1], "dict not sorted at {:?}", w);
        }
    }

    #[test]
    fn test_speedtest_and_different() {
        let mut cfg = crate::config::ClakConfig::default();
        cfg.spelling.auto_restore = true;
        cfg.spelling.modern_tone = false;

        let mut ctx = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx.apply_config(&cfg);
        let mut ctx_closes = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx_closes.apply_config(&cfg);
        let mut sim_closes = String::new();
        for ch in "closes".chars() {
            let s = ch.to_string();
            let act = ctx_closes.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_closes.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_closes.push_str(commit);
            } else if act.action_type == 0 {
                sim_closes.push(ch);
            }
        }
        assert_eq!(sim_closes, "closes");

        let mut sim_text = String::new();
        for ch in "speedtest".chars() {
            let s = ch.to_string();
            let act = ctx.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_text.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_text.push_str(commit);
            } else if act.action_type == 0 {
                sim_text.push(ch);
            }
        }
        assert_eq!(sim_text, "speedtest");

        let mut ctx2 = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx2.apply_config(&cfg);
        let mut sim_text2 = String::new();
        for ch in "different".chars() {
            let s = ch.to_string();
            let act = ctx2.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_text2.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_text2.push_str(commit);
            } else if act.action_type == 0 {
                sim_text2.push(ch);
            }
        }
        assert_eq!(sim_text2, "different");

        let mut ctx_msg = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx_msg.apply_config(&cfg);
        let mut sim_msg = String::new();
        for ch in "messenger".chars() {
            let s = ch.to_string();
            let act = ctx_msg.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_msg.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_msg.push_str(commit);
            } else if act.action_type == 0 {
                sim_msg.push(ch);
            }
        }
        assert_eq!(sim_msg, "messenger");

        let mut ctx3 = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx3.apply_config(&cfg);
        let mut sim_text3 = String::new();
        for ch in "diffferent".chars() {
            let s = ch.to_string();
            let act = ctx3.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_text3.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_text3.push_str(commit);
            } else if act.action_type == 0 {
                sim_text3.push(ch);
            }
        }
        assert_eq!(sim_text3, "different");

        let mut ctx_before = crate::ime::ClakContext::new(crate::Method::Telex);
        ctx_before.apply_config(&cfg);
        let mut sim_before = String::new();
        for ch in "beffore".chars() {
            let s = ch.to_string();
            let act = ctx_before.process_key(ch as u32, &s, false, None, 0, 0);
            if act.action_type == 4 {
                let del = act.delete_count;
                for _ in 0..del {
                    sim_before.pop();
                }
                let commit = unsafe {
                    if act.commit_str.is_null() {
                        ""
                    } else {
                        std::ffi::CStr::from_ptr(act.commit_str).to_str().unwrap()
                    }
                };
                sim_before.push_str(commit);
            } else if act.action_type == 0 {
                sim_before.push(ch);
            }
        }
        assert_eq!(sim_before, "before");
    }

    #[test]
    fn test_informal_and_loanwords_with_spelling_enabled() {
        let e = eng(true, true);
        assert_eq!(e.transform("kos"), "kó");
        assert_eq!(e.transform("kcos"), "kcó");
        assert_eq!(e.transform("ddcuj"), "đcụ");
        assert_eq!(e.transform("klaf"), "klà");
        assert_eq!(e.transform("kphari"), "kphải");
        assert_eq!(e.transform("croong"), "crông");
        assert_eq!(e.transform("kpaang"), "kpâng");
        assert_eq!(e.transform("kpawng"), "kpăng");
        assert_eq!(e.transform("ddrawk"), "đrăk");
        assert_eq!(e.transform("drawk"), "drăk");
        assert_eq!(e.transform("ookee"), "ôkê");

        // english protection intact
        assert_eq!(e.transform("ghost"), "ghost");
        assert_eq!(e.transform("ghostty"), "ghostty");
        assert_eq!(e.transform("detect"), "detect");
        assert_eq!(e.transform("speedtest"), "speedtest");
        assert_eq!(e.transform("class"), "class");
        assert_eq!(e.transform("address"), "address");
        assert_eq!(e.transform("draf"), "draf");
        assert_eq!(e.transform("draft"), "draft");
        assert_eq!(e.transform("prof"), "prof");
        assert_eq!(e.transform("bras"), "bras");
        assert_eq!(e.transform("fres"), "fres");
    }
}
