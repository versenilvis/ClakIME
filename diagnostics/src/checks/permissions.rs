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
