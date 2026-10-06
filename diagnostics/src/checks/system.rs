use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::CStr;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub kernel_release: String,
    pub architecture: String,
    pub session_type: String,
    pub desktop_environment: String,
}

pub fn check_system() -> SystemInfo {
    let mut os_name = String::new();
    let mut kernel_release = String::new();
    let mut architecture = String::new();

    unsafe {
        let mut uts: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut uts) == 0 {
            os_name = CStr::from_ptr(uts.sysname.as_ptr())
                .to_string_lossy()
                .into_owned();
            kernel_release = CStr::from_ptr(uts.release.as_ptr())
                .to_string_lossy()
                .into_owned();
            architecture = CStr::from_ptr(uts.machine.as_ptr())
                .to_string_lossy()
                .into_owned();
        }
    }

    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                let pretty = val.trim_matches('"').trim();
                if !pretty.is_empty() {
                    os_name = pretty.to_string();
                }
                break;
            }
        }
    }

    let session_type = env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".to_string());
    let desktop_environment =
        env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".to_string());

    SystemInfo {
        os_name,
        kernel_release,
        architecture,
        session_type,
        desktop_environment,
    }
}
