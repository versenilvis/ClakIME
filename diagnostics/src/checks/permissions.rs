use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionChecks {
    pub uinput_writable: bool,
    pub xtest_available: bool,
    pub input_readable_count: usize,
    pub input_total_count: usize,
}

fn check_xtest_available() -> bool {
    let display = match std::env::var("DISPLAY") {
        Ok(d) if !d.is_empty() => d,
        _ => return false,
    };
    let _ = display;
    let c_xcb1 = CString::new("libxcb.so.1").unwrap();
    let c_xcb2 = CString::new("libxcb.so").unwrap();
    let c_xtest1 = CString::new("libxcb-xtest.so.0").unwrap();
    let c_xtest2 = CString::new("libxcb-xtest.so").unwrap();

    unsafe {
        let mut xcb = libc::dlopen(c_xcb1.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if xcb.is_null() {
            xcb = libc::dlopen(c_xcb2.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        }
        let mut xtest = libc::dlopen(c_xtest1.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if xtest.is_null() {
            xtest = libc::dlopen(c_xtest2.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        }

        let ok = !xcb.is_null() && !xtest.is_null();
        if !xcb.is_null() {
            libc::dlclose(xcb);
        }
        if !xtest.is_null() {
            libc::dlclose(xtest);
        }
        ok
    }
}

pub fn check_permissions() -> PermissionChecks {
    let uinput_path = CString::new("/dev/uinput").unwrap();
    // check write access to uinput virtual device
    let uinput_writable = unsafe { libc::access(uinput_path.as_ptr(), libc::W_OK) == 0 };
    let xtest_available = check_xtest_available();

    let mut input_total_count = 0;
    let mut input_readable_count = 0;

    if let Ok(entries) = fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.starts_with("event") {
                input_total_count += 1;
                let path = entry.path();
                if let Ok(c_path) = CString::new(path.to_string_lossy().as_bytes()) {
                    let readable = unsafe { libc::access(c_path.as_ptr(), libc::R_OK) == 0 };
                    if readable {
                        input_readable_count += 1;
                    }
                }
            }
        }
    }

    PermissionChecks {
        uinput_writable,
        xtest_available,
        input_readable_count,
        input_total_count,
    }
}

fn get_current_username() -> Result<String, String> {
    unsafe {
        let uid = libc::getuid();
        let pwd = libc::getpwuid(uid);
        if pwd.is_null() || (*pwd).pw_name.is_null() {
            return Err("không xác định được username từ UID".to_string());
        }
        let c_str = std::ffi::CStr::from_ptr((*pwd).pw_name);
        let name = c_str.to_string_lossy().into_owned();
        validate_username(&name)?;
        Ok(name)
    }
}

pub fn validate_username(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 32 {
        return Err("username rỗng hoặc vượt quá 32 ký tự".to_string());
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() && first != '_' {
        return Err("ký tự đầu username không hợp lệ".to_string());
    }
    for c in chars {
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_' && c != '-' && c != '.' && c != '$' {
            return Err("username chứa ký tự không an toàn".to_string());
        }
    }
    Ok(())
}

pub fn fix_input_permissions() -> Result<(), String> {
    let user = get_current_username()?;

    let script = r#"mkdir -p /etc/udev/rules.d /etc/modules-load.d && \
printf '%s\n' 'KERNEL=="uinput", SUBSYSTEM=="misc", OPTIONS+="static_node=uinput", MODE="0660", GROUP="input", TAG+="uaccess"' \
'SUBSYSTEM=="input", ENV{ID_INPUT_MOUSE}=="1", TAG+="uaccess"' \
'SUBSYSTEM=="input", ENV{ID_INPUT_TOUCHPAD}=="1", TAG+="uaccess"' > /etc/udev/rules.d/99-uinput.rules && \
echo 'uinput' > /etc/modules-load.d/uinput.conf && \
modprobe uinput 2>/dev/null || true; \
udevadm control --reload-rules 2>/dev/null || true; \
udevadm trigger --name-match=uinput 2>/dev/null || udevadm trigger /dev/uinput 2>/dev/null || true; \
udevadm trigger --subsystem-match=input 2>/dev/null || true; \
target_user="$1"; \
[ -n "$target_user" ] && usermod -aG input "$target_user" 2>/dev/null || true; \
if command -v setfacl >/dev/null 2>&1 && [ -n "$target_user" ]; then \
  setfacl -m "u:$target_user:rw" /dev/uinput 2>/dev/null || true; \
  for ev in /dev/input/event*; do \
    [ -e "$ev" ] && setfacl -m "u:$target_user:rw" "$ev" 2>/dev/null || true; \
  done; \
fi"#;

    let pkexec_status = std::process::Command::new("pkexec")
        .args(["sh", "-c", script, "--", &user])
        .status();

    if let Ok(status) = pkexec_status {
        if status.success() {
            return Ok(());
        }
    }

    let sudo_status = std::process::Command::new("sudo")
        .args(["sh", "-c", script, "--", &user])
        .status();

    if let Ok(status) = sudo_status {
        if status.success() {
            return Ok(());
        }
    }

    Err("không thể cấp quyền: cần quyền quản trị root qua pkexec hoặc sudo".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_username() {
        assert!(validate_username("verse").is_ok());
        assert!(validate_username("_clak").is_ok());
        assert!(validate_username("user-123").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("user;rm -rf /").is_err());
        assert!(validate_username("user\"").is_err());
        assert!(validate_username("user$").is_ok());
    }

    #[test]
    fn test_get_current_username() {
        let name = get_current_username();
        assert!(name.is_ok());
    }
}
