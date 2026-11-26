use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize, Default)]
struct DynamicMacroFile {
    #[serde(default)]
    macros: HashMap<String, String>,
}

/// Caches user-defined dynamic macros loaded from dynamic_macros.json
#[derive(Default)]
pub struct DynamicMacroStore {
    cached_mtime: Option<SystemTime>,
    cached_path: Option<PathBuf>,
    pub custom_macros: HashMap<String, String>,
}

impl DynamicMacroStore {
    fn config_path() -> Option<PathBuf> {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("clak/dynamic_macros.json"));
            }
        }
        std::env::var("HOME")
            .ok()
            .filter(|h| !h.is_empty())
            .map(|h| PathBuf::from(h).join(".config/clak/dynamic_macros.json"))
    }

    /// Reloads dynamic_macros.json if the file was modified since the last read
    pub fn refresh(&mut self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        let Ok(meta) = fs::metadata(&path) else {
            self.custom_macros.clear();
            self.cached_mtime = None;
            self.cached_path = Some(path);
            return;
        };
        let mtime = meta.modified().ok();
        if self.cached_path.as_ref() == Some(&path) && self.cached_mtime == mtime {
            return;
        }
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<DynamicMacroFile>(&raw) {
                self.custom_macros = parsed
                    .macros
                    .into_iter()
                    .map(|(k, v)| (k.trim().to_lowercase(), v))
                    .collect();
            }
        }
        self.cached_mtime = mtime;
        self.cached_path = Some(path);
    }

    /// Resolves `:inner:` to its expanded replacement using user config, JSON file, or built-ins
    pub fn resolve(
        &mut self,
        inner: &str,
        extra_macros: &HashMap<String, String>,
    ) -> Option<String> {
        let trimmed = inner.trim();
        if !trimmed.is_empty() {
            let lower = trimmed.to_lowercase();
            if let Some(template) = extra_macros.get(&lower) {
                return Some(render_template(template));
            }
        }
        self.refresh();
        resolve_dynamic_macro(inner, &self.custom_macros)
    }
}

/// Returns built-in `:command:` dynamic macro entries for UI display
pub fn builtin_dynamic_macro_docs() -> &'static [(&'static str, &'static str)] {
    &[
        (":ngay:", "Ngày hiện tại (dd/mm/yyyy)"),
        (":ngay iso:", "Ngày chuẩn ISO (yyyy-mm-dd)"),
        (":ngay +1:", "Ngày cộng/trừ (±N ngày, hoặc :ngay mai:)"),
        (":ngay chu:", "Ngày dạng chữ (ngày ... tháng ... năm ...)"),
        (":gio:", "Giờ hiện tại (hh:mm, hoặc :gio s:)"),
        (":ngay gio:", "Ngày và giờ (dd/mm/yyyy hh:mm)"),
        (":thu:", "Thứ trong tuần (Thứ Hai..Chủ Nhật)"),
        (":tinh 1+2:", "Tính biểu thức toán (+ - * / %)"),
        (":uuid:", "Sinh UUID v4 ngẫu nhiên"),
        (":pin:", "Mã 6 số ngẫu nhiên (hoặc :rand N:)"),
    ]
}

/// Extracts `(full_colon_token, inner_command)` if `before_cursor` ends with a valid `:...:` token
pub fn extract_colon_macro_at_end(before_cursor: &str) -> Option<(&str, &str)> {
    if !before_cursor.ends_with(':') || before_cursor.len() < 3 {
        return None;
    }
    let without_end = &before_cursor[..before_cursor.len() - 1];
    let open_idx = without_end.rfind(':')?;
    let inner = &without_end[open_idx + 1..];
    if inner.is_empty() || inner.contains('\n') || inner.contains('\r') {
        return None;
    }
    // reject ::foo: or :foo::
    if inner.starts_with(':') || inner.ends_with(':') {
        return None;
    }
    // require start-of-string or whitespace immediately before the opening colon
    if open_idx > 0 {
        let prev_ch = before_cursor[..open_idx].chars().next_back()?;
        if !prev_ch.is_whitespace() {
            return None;
        }
    }
    let full = &before_cursor[open_idx..];
    Some((full, inner))
}

/// Resolves built-in dynamic macros and custom JSON templates for `inner`
pub fn resolve_dynamic_macro(
    inner: &str,
    custom_macros: &HashMap<String, String>,
) -> Option<String> {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();

    if let Some(template) = custom_macros.get(&lower) {
        return Some(render_template(template));
    }

    if let Some(expr) = trimmed.strip_prefix('=') {
        return eval_math_expr(expr);
    }
    for prefix in ["calc ", "tinh ", "tính "] {
        if lower.starts_with(prefix) {
            return eval_math_expr(&trimmed[prefix.len()..]);
        }
    }

    if let Some(res) = resolve_datetime_macro(&lower) {
        return Some(res);
    }

    if lower == "uuid" {
        return Some(generate_uuid_v4());
    }
    if lower == "pin" {
        return Some(generate_digits(6));
    }
    if let Some(rest) = lower.strip_prefix("rand ") {
        let n = rest.trim().parse::<usize>().ok()?.clamp(1, 32);
        return Some(generate_digits(n));
    }

    None
}

fn local_tm(offset_days: i64) -> Option<libc::tm> {
    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let target: libc::time_t = now_secs.saturating_add(offset_days.saturating_mul(86_400));
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ptr = unsafe { libc::localtime_r(&target, &mut tm) };
    if ptr.is_null() {
        None
    } else {
        Some(tm)
    }
}

fn format_date_dmy(tm: &libc::tm) -> String {
    format!(
        "{:02}/{:02}/{:04}",
        tm.tm_mday,
        tm.tm_mon + 1,
        tm.tm_year + 1900
    )
}

fn format_date_iso(tm: &libc::tm) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday
    )
}

fn format_time_hm(tm: &libc::tm) -> String {
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

fn format_time_hms(tm: &libc::tm) -> String {
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

fn format_weekday_vi(tm: &libc::tm) -> &'static str {
    match tm.tm_wday {
        0 => "Chủ Nhật",
        1 => "Thứ Hai",
        2 => "Thứ Ba",
        3 => "Thứ Tư",
        4 => "Thứ Năm",
        5 => "Thứ Sáu",
        6 => "Thứ Bảy",
        _ => "",
    }
}

fn resolve_datetime_macro(lower: &str) -> Option<String> {
    if matches!(
        lower,
        "date" | "ngay" | "ngày" | "hom nay" | "hôm nay" | "today"
    ) {
        return local_tm(0).map(|tm| format_date_dmy(&tm));
    }
    if matches!(lower, "ngay mai" | "ngày mai" | "tomorrow") {
        return local_tm(1).map(|tm| format_date_dmy(&tm));
    }
    if matches!(lower, "hom qua" | "hôm qua" | "yesterday") {
        return local_tm(-1).map(|tm| format_date_dmy(&tm));
    }
    if matches!(lower, "date iso" | "ngay iso" | "ngày iso" | "iso") {
        return local_tm(0).map(|tm| format_date_iso(&tm));
    }
    if matches!(lower, "ngay chu" | "ngày chữ" | "date full") {
        return local_tm(0).map(|tm| {
            format!(
                "ngày {} tháng {} năm {}",
                tm.tm_mday,
                tm.tm_mon + 1,
                tm.tm_year + 1900
            )
        });
    }
    for prefix in ["ngay ", "ngày ", "date "] {
        if let Some(arg) = lower.strip_prefix(prefix) {
            let arg = arg.trim();
            if arg.starts_with('+') || arg.starts_with('-') {
                if let Ok(days) = arg.parse::<i64>() {
                    return local_tm(days.clamp(-36500, 36500)).map(|tm| format_date_dmy(&tm));
                }
            }
        }
    }
    if matches!(
        lower,
        "time" | "gio" | "giờ" | "bay gio" | "bây giờ" | "now"
    ) {
        return local_tm(0).map(|tm| format_time_hm(&tm));
    }
    if matches!(lower, "time s" | "gio s" | "giờ s") {
        return local_tm(0).map(|tm| format_time_hms(&tm));
    }
    if matches!(lower, "datetime" | "ngay gio" | "ngày giờ") {
        return local_tm(0).map(|tm| format!("{} {}", format_date_dmy(&tm), format_time_hm(&tm)));
    }
    if matches!(lower, "thu" | "thứ" | "weekday") {
        return local_tm(0).map(|tm| format_weekday_vi(&tm).to_string());
    }
    if matches!(lower, "timestamp" | "ts") {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        return Some(secs.to_string());
    }
    None
}

fn render_template(template: &str) -> String {
    let mut out = template.to_string();
    if let Some(tm) = local_tm(0) {
        out = out.replace("{date}", &format_date_dmy(&tm));
        out = out.replace("{date_iso}", &format_date_iso(&tm));
        out = out.replace("{time}", &format_time_hm(&tm));
        out = out.replace("{time_s}", &format_time_hms(&tm));
        out = out.replace(
            "{datetime}",
            &format!("{} {}", format_date_dmy(&tm), format_time_hm(&tm)),
        );
        out = out.replace("{weekday}", format_weekday_vi(&tm));
    }
    if out.contains("{uuid}") {
        out = out.replace("{uuid}", &generate_uuid_v4());
    }
    if out.contains("{timestamp}") {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        out = out.replace("{timestamp}", &secs.to_string());
    }
    out
}

fn random_bytes(buf: &mut [u8]) {
    if let Ok(mut f) = File::open("/dev/urandom") {
        if f.read_exact(buf).is_ok() {
            return;
        }
    }
    let mut state = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15);
    for b in buf.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *b = (state & 0xff) as u8;
    }
}

fn generate_uuid_v4() -> String {
    let mut b = [0u8; 16];
    random_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

fn generate_digits(count: usize) -> String {
    let mut buf = vec![0u8; count];
    random_bytes(&mut buf);
    buf.into_iter()
        .map(|x| char::from(b'0' + (x % 10)))
        .collect()
}

struct MathParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> MathParser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn parse_expr(&mut self) -> Option<f64> {
        let mut val = self.parse_term()?;
        loop {
            self.skip_ws();
            match self.bytes.get(self.pos).copied() {
                Some(b'+') => {
                    self.pos += 1;
                    val += self.parse_term()?;
                }
                Some(b'-') => {
                    self.pos += 1;
                    val -= self.parse_term()?;
                }
                _ => break,
            }
        }
        Some(val)
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut val = self.parse_factor()?;
        loop {
            self.skip_ws();
            match self.bytes.get(self.pos).copied() {
                Some(b'*') => {
                    self.pos += 1;
                    val *= self.parse_factor()?;
                }
                Some(b'/') => {
                    self.pos += 1;
                    let rhs = self.parse_factor()?;
                    if rhs == 0.0 {
                        return None;
                    }
                    val /= rhs;
                }
                Some(b'%') => {
                    self.pos += 1;
                    let rhs = self.parse_factor()?;
                    if rhs == 0.0 {
                        return None;
                    }
                    val %= rhs;
                }
                _ => break,
            }
        }
        Some(val)
    }

    fn parse_factor(&mut self) -> Option<f64> {
        self.skip_ws();
        match self.bytes.get(self.pos).copied()? {
            b'+' => {
                self.pos += 1;
                self.parse_factor()
            }
            b'-' => {
                self.pos += 1;
                self.parse_factor().map(|v| -v)
            }
            b'(' => {
                self.pos += 1;
                let v = self.parse_expr()?;
                self.skip_ws();
                if self.bytes.get(self.pos).copied() != Some(b')') {
                    return None;
                }
                self.pos += 1;
                Some(v)
            }
            _ => self.parse_number(),
        }
    }

    fn parse_number(&mut self) -> Option<f64> {
        self.skip_ws();
        let start = self.pos;
        let mut seen_digit = false;
        let mut seen_dot = false;
        while self.pos < self.bytes.len() {
            let c = self.bytes[self.pos];
            if c.is_ascii_digit() {
                seen_digit = true;
                self.pos += 1;
            } else if c == b'.' && !seen_dot {
                seen_dot = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        if !seen_digit {
            return None;
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos]).ok()?;
        s.parse::<f64>().ok()
    }
}

fn eval_math_expr(expr: &str) -> Option<String> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parser = MathParser {
        bytes: trimmed.as_bytes(),
        pos: 0,
    };
    let val = parser.parse_expr()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() || !val.is_finite() {
        return None;
    }
    if (val - val.round()).abs() < 1e-9 && val.abs() < 1e15 {
        Some(format!("{}", val.round() as i64))
    } else {
        let s = format!("{:.6}", val);
        Some(s.trim_end_matches('0').trim_end_matches('.').to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_colon_macro_respects_boundaries() {
        assert_eq!(
            extract_colon_macro_at_end(":hom nay:"),
            Some((":hom nay:", "hom nay"))
        );
        assert_eq!(
            extract_colon_macro_at_end("xin chao :ngay +7:"),
            Some((":ngay +7:", "ngay +7"))
        );
        assert_eq!(extract_colon_macro_at_end("std::cout:"), None);
        assert_eq!(extract_colon_macro_at_end("abc:date:"), None);
        assert_eq!(extract_colon_macro_at_end("::"), None);
        assert_eq!(extract_colon_macro_at_end(":a\n:b:"), Some((":b:", "b")));
    }

    #[test]
    fn test_builtin_dynamic_macros() {
        let empty = HashMap::new();
        let date = resolve_dynamic_macro("date", &empty).unwrap();
        assert_eq!(date.len(), 10);
        assert_eq!(&date[2..3], "/");

        let iso = resolve_dynamic_macro("ngay iso", &empty).unwrap();
        assert_eq!(iso.len(), 10);
        assert_eq!(&iso[4..5], "-");

        let tomorrow = resolve_dynamic_macro("ngay +1", &empty).unwrap();
        assert_ne!(tomorrow, date);

        let uuid = resolve_dynamic_macro("uuid", &empty).unwrap();
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[14..15], "4");

        assert_eq!(resolve_dynamic_macro("rand 8", &empty).unwrap().len(), 8);
        assert_eq!(
            resolve_dynamic_macro("tinh 125*38", &empty).as_deref(),
            Some("4750")
        );
        assert_eq!(
            resolve_dynamic_macro("= (10 + 5) / 2", &empty).as_deref(),
            Some("7.5")
        );
        assert_eq!(resolve_dynamic_macro("cout << a", &empty), None);
    }

    #[test]
    fn test_custom_json_template_macro() {
        let mut custom = HashMap::new();
        custom.insert("gui khach".to_string(), "Hom nay {date_iso}".to_string());
        let res = resolve_dynamic_macro("Gui Khach", &custom).unwrap();
        assert!(res.starts_with("Hom nay "));
        assert_eq!(res.len(), "Hom nay 2026-11-26".len());
    }
}
