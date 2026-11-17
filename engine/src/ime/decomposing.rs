use crate::Method;

fn decompose_char_to_telex(c: char) -> (&'static str, Option<char>) {
    match c {
        'á' => ("a", Some('s')),
        'à' => ("a", Some('f')),
        'ả' => ("a", Some('r')),
        'ã' => ("a", Some('x')),
        'ạ' => ("a", Some('j')),
        'Á' => ("A", Some('s')),
        'À' => ("A", Some('f')),
        'Ả' => ("A", Some('r')),
        'Ã' => ("A", Some('x')),
        'Ạ' => ("A", Some('j')),
        'â' => ("aa", None),
        'ấ' => ("aa", Some('s')),
        'ầ' => ("aa", Some('f')),
        'ẩ' => ("aa", Some('r')),
        'ẫ' => ("aa", Some('x')),
        'ậ' => ("aa", Some('j')),
        'Â' => ("AA", None),
        'Ấ' => ("AA", Some('s')),
        'Ầ' => ("AA", Some('f')),
        'Ẩ' => ("AA", Some('r')),
        'Ẫ' => ("AA", Some('x')),
        'Ậ' => ("AA", Some('j')),
        'ă' => ("aw", None),
        'ắ' => ("aw", Some('s')),
        'ằ' => ("aw", Some('f')),
        'ẳ' => ("aw", Some('r')),
        'ẵ' => ("aw", Some('x')),
        'ặ' => ("aw", Some('j')),
        'Ă' => ("AW", None),
        'Ắ' => ("AW", Some('s')),
        'Ằ' => ("AW", Some('f')),
        'Ẳ' => ("AW", Some('r')),
        'Ẵ' => ("AW", Some('x')),
        'Ặ' => ("AW", Some('j')),
        'é' => ("e", Some('s')),
        'è' => ("e", Some('f')),
        'ẻ' => ("e", Some('r')),
        'ẽ' => ("e", Some('x')),
        'ẹ' => ("e", Some('j')),
        'É' => ("E", Some('s')),
        'È' => ("E", Some('f')),
        'Ẻ' => ("E", Some('r')),
        'Ẽ' => ("E", Some('x')),
        'Ẹ' => ("E", Some('j')),
        'ê' => ("ee", None),
        'ế' => ("ee", Some('s')),
        'ề' => ("ee", Some('f')),
        'ể' => ("ee", Some('r')),
        'ễ' => ("ee", Some('x')),
        'ệ' => ("ee", Some('j')),
        'Ê' => ("EE", None),
        'Ế' => ("EE", Some('s')),
        'Ề' => ("EE", Some('f')),
        'Ể' => ("EE", Some('r')),
        'Ễ' => ("EE", Some('x')),
        'Ệ' => ("EE", Some('j')),
        'í' => ("i", Some('s')),
        'ì' => ("i", Some('f')),
        'ỉ' => ("i", Some('r')),
        'ĩ' => ("i", Some('x')),
        'ị' => ("i", Some('j')),
        'Í' => ("I", Some('s')),
        'Ì' => ("I", Some('f')),
        'Ỉ' => ("I", Some('r')),
        'Ĩ' => ("I", Some('x')),
        'Ị' => ("I", Some('j')),
        'ó' => ("o", Some('s')),
        'ò' => ("o", Some('f')),
        'ỏ' => ("o", Some('r')),
        'õ' => ("o", Some('x')),
        'ọ' => ("o", Some('j')),
        'Ó' => ("O", Some('s')),
        'Ò' => ("O", Some('f')),
        'Ỏ' => ("O", Some('r')),
        'Õ' => ("O", Some('x')),
        'Ọ' => ("O", Some('j')),
        'ô' => ("oo", None),
        'ố' => ("oo", Some('s')),
        'ồ' => ("oo", Some('f')),
        'ổ' => ("oo", Some('r')),
        'ỗ' => ("oo", Some('x')),
        'ộ' => ("oo", Some('j')),
        'Ô' => ("OO", None),
        'Ố' => ("OO", Some('s')),
        'Ồ' => ("OO", Some('f')),
        'Ổ' => ("OO", Some('r')),
        'Ỗ' => ("OO", Some('x')),
        'Ộ' => ("OO", Some('j')),
        'ơ' => ("ow", None),
        'ớ' => ("ow", Some('s')),
        'ờ' => ("ow", Some('f')),
        'ở' => ("ow", Some('r')),
        'ỡ' => ("ow", Some('x')),
        'ợ' => ("ow", Some('j')),
        'Ơ' => ("OW", None),
        'Ớ' => ("OW", Some('s')),
        'Ờ' => ("OW", Some('f')),
        'Ở' => ("OW", Some('r')),
        'Ỡ' => ("OW", Some('x')),
        'Ợ' => ("OW", Some('j')),
        'ú' => ("u", Some('s')),
        'ù' => ("u", Some('f')),
        'ủ' => ("u", Some('r')),
        'ũ' => ("u", Some('x')),
        'ụ' => ("u", Some('j')),
        'Ú' => ("U", Some('s')),
        'Ù' => ("U", Some('f')),
        'Ủ' => ("U", Some('r')),
        'Ũ' => ("U", Some('x')),
        'Ụ' => ("U", Some('j')),
        'ư' => ("uw", None),
        'ứ' => ("uw", Some('s')),
        'ừ' => ("uw", Some('f')),
        'ử' => ("uw", Some('r')),
        'ữ' => ("uw", Some('x')),
        'ự' => ("uw", Some('j')),
        'Ư' => ("UW", None),
        'Ứ' => ("UW", Some('s')),
        'Ừ' => ("UW", Some('f')),
        'Ử' => ("UW", Some('r')),
        'Ữ' => ("UW", Some('x')),
        'Ự' => ("UW", Some('j')),
        'ý' => ("y", Some('s')),
        'ỳ' => ("y", Some('f')),
        'ỷ' => ("y", Some('r')),
        'ỹ' => ("y", Some('x')),
        'ỵ' => ("y", Some('j')),
        'Ý' => ("Y", Some('s')),
        'Ỳ' => ("Y", Some('f')),
        'Ỷ' => ("Y", Some('r')),
        'Ỹ' => ("Y", Some('x')),
        'Ỵ' => ("Y", Some('j')),
        'đ' => ("dd", None),
        'Đ' => ("Dd", None),
        _ => ("", None),
    }
}

pub fn decompose_to_telex(word: &str) -> String {
    let mut result = String::with_capacity(word.len() + 4);
    let mut trailing_tone: Option<char> = None;

    for ch in word.chars() {
        let (base, tone) = decompose_char_to_telex(ch);
        if !base.is_empty() {
            result.push_str(base);
            if tone.is_some() {
                trailing_tone = tone;
            }
        } else {
            result.push(ch);
        }
    }

    if let Some(t) = trailing_tone {
        result.push(t);
    }

    result
}

pub fn is_modifier_for_method(method: Method, key_ch: char) -> bool {
    let lower = key_ch.to_ascii_lowercase();
    let telex = matches!(
        lower,
        'a' | 'e' | 'o' | 'd' | 'w' | 's' | 'f' | 'r' | 'x' | 'j' | 'z'
    );
    match method {
        Method::Telex => telex,
        Method::Vni => lower.is_ascii_digit(),
        Method::TeipVni => telex || lower.is_ascii_digit(),
        Method::Viqr => false,
    }
}

pub fn decompose_to_vni(word: &str) -> String {
    let mut result = String::with_capacity(word.len() + 4);
    let mut trailing_tone: Option<char> = None;

    for ch in word.chars() {
        let (base, tone) = decompose_char_to_telex(ch);
        if base.is_empty() {
            result.push(ch);
            continue;
        }
        let mut chars = base.chars();
        let first = chars.next().unwrap_or(ch);
        let marker = match base.to_ascii_lowercase().as_str() {
            "aa" | "ee" | "oo" => Some('6'),
            "ow" | "uw" => Some('7'),
            "aw" => Some('8'),
            "dd" => Some('9'),
            _ => None,
        };
        result.push(first);
        if let Some(m) = marker {
            result.push(m);
        } else {
            result.extend(chars);
        }
        if let Some(t) = tone {
            trailing_tone = Some(match t {
                's' => '1',
                'f' => '2',
                'r' => '3',
                'x' => '4',
                _ => '5',
            });
        }
    }

    if let Some(t) = trailing_tone {
        result.push(t);
    }

    result
}

pub fn decompose_to_raw(word: &str, method: Method) -> String {
    match method {
        Method::Vni | Method::TeipVni => decompose_to_vni(word),
        _ => decompose_to_telex(word),
    }
}

pub fn to_base_ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a' | 'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ'
            | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' => 'a',
            'A' | 'Á' | 'À' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ắ' | 'Ằ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Â' | 'Ấ'
            | 'Ầ' | 'Ẩ' | 'Ẫ' | 'Ậ' => 'A',
            'e' | 'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => 'e',
            'E' | 'É' | 'È' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ế' | 'Ề' | 'Ể' | 'Ễ' | 'Ệ' => 'E',
            'i' | 'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
            'I' | 'Í' | 'Ì' | 'Ỉ' | 'Ĩ' | 'Ị' => 'I',
            'o' | 'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ'
            | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
            'O' | 'Ó' | 'Ò' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ố' | 'Ồ' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ơ' | 'Ớ'
            | 'Ờ' | 'Ở' | 'Ỡ' | 'Ợ' => 'O',
            'u' | 'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => 'u',
            'U' | 'Ú' | 'Ù' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ứ' | 'Ừ' | 'Ử' | 'Ữ' | 'Ự' => 'U',
            'y' | 'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
            'Y' | 'Ý' | 'Ỳ' | 'Ỷ' | 'Ỹ' | 'Ỵ' => 'Y',
            'đ' => 'd',
            'Đ' => 'D',
            other => other,
        })
        .collect()
}

pub fn compare_and_split(a: &str, b: &str) -> (String, String) {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let mut prefix_len = 0;
    while prefix_len < a_chars.len()
        && prefix_len < b_chars.len()
        && a_chars[prefix_len] == b_chars[prefix_len]
    {
        prefix_len += 1;
    }

    let deleted: String = a_chars[prefix_len..].iter().collect();
    let added: String = b_chars[prefix_len..].iter().collect();
    (deleted, added)
}
