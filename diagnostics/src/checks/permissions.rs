use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionChecks {
    pub uinput_writable: bool,
    pub input_readable_count: usize,
    pub input_total_count: usize,
}

pub fn check_permissions() -> PermissionChecks {
    let uinput_path = CString::new("/dev/uinput").unwrap();
    // check write access to uinput virtual device
    let uinput_writable = unsafe { libc::access(uinput_path.as_ptr(), libc::W_OK) == 0 };

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
        input_readable_count,
        input_total_count,
    }
}

pub fn fix_input_permissions() -> Result<(), String> {
    let user = std::env::var("USER").map_err(|e| e.to_string())?;
    if user.is_empty() {
        return Err("Không xác định được biến USER".to_string());
    }

    let script = format!(
        r#"mkdir -p /etc/udev/rules.d /etc/modules-load.d && \
printf '%s\n' 'KERNEL=="uinput", SUBSYSTEM=="misc", OPTIONS+="static_node=uinput", MODE="0660", GROUP="input", TAG+="uaccess"' \
'SUBSYSTEM=="input", ENV{{ID_INPUT_MOUSE}}=="1", TAG+="uaccess"' \
'SUBSYSTEM=="input", ENV{{ID_INPUT_TOUCHPAD}}=="1", TAG+="uaccess"' > /etc/udev/rules.d/99-uinput.rules && \
echo 'uinput' > /etc/modules-load.d/uinput.conf && \
modprobe uinput 2>/dev/null || true; \
udevadm control --reload-rules 2>/dev/null || true; \
udevadm trigger --name-match=uinput 2>/dev/null || udevadm trigger /dev/uinput 2>/dev/null || true; \
udevadm trigger --subsystem-match=input 2>/dev/null || true; \
usermod -aG input "{user}" 2>/dev/null || true; \
if command -v setfacl >/dev/null 2>&1; then \
  setfacl -m "u:{user}:rw" /dev/uinput 2>/dev/null || true; \
  for ev in /dev/input/event*; do \
    [ -e "$ev" ] && setfacl -m "u:{user}:rw" "$ev" 2>/dev/null || true; \
  done; \
fi"#,
        user = user
    );

    let pkexec_status = std::process::Command::new("pkexec")
        .args(["sh", "-c", &script])
        .status();

    if let Ok(status) = pkexec_status {
        if status.success() {
            return Ok(());
        }
    }

    let sudo_status = std::process::Command::new("sudo")
        .args(["sh", "-c", &script])
        .status();

    if let Ok(status) = sudo_status {
        if status.success() {
            return Ok(());
        }
    }

    Err("Không thể cấp quyền: cần quyền quản trị root qua pkexec hoặc sudo".to_string())
}
