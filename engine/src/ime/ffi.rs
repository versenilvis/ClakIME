use crate::ime::action::{ImeAction, ACTION_FORWARD};
use crate::ime::ClakContext;
use crate::Method;
use std::ffi::CStr;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_context_new(method: i32) -> *mut ClakContext {
    let m = match method {
        0 => Method::Telex,
        1 => Method::Vni,
        2 => Method::Viqr,
        3 => Method::TeipVni,
        _ => Method::Telex,
    };
    Box::into_raw(Box::new(ClakContext::new(m)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_context_free(ctx: *mut ClakContext) {
    if !ctx.is_null() {
        drop(Box::from_raw(ctx));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_is_composing(ctx: *const ClakContext) -> bool {
    if let Some(c) = ctx.as_ref() {
        !c.last_composed.is_empty() || !c.raw_buffer.is_empty()
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_context_reset(ctx: *mut ClakContext) {
    if let Some(c) = ctx.as_mut() {
        c.reset();
        c.uncommit_candidate = None;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_context_apply_config(
    ctx: *mut ClakContext,
    cfg: *const crate::config::ClakConfig,
) {
    if let (Some(c), Some(config)) = (ctx.as_mut(), cfg.as_ref()) {
        c.apply_config(config);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clak_process_key(
    ctx: *mut ClakContext,
    key_sym: u32,
    key_str: *const c_char,
    has_ctrl_alt: bool,
    surrounding_text: *const c_char,
    cursor: usize,
    anchor: usize,
) -> ImeAction {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let c = match ctx.as_mut() {
            Some(ptr) => ptr,
            None => {
                return ImeAction {
                    action_type: ACTION_FORWARD,
                    delete_count: 0,
                    commit_str: std::ptr::null(),
                    delete_str: std::ptr::null(),
                }
            }
        };

        let k_str = if key_str.is_null() {
            ""
        } else {
            CStr::from_ptr(key_str).to_str().unwrap_or_default()
        };

        let s_text = if surrounding_text.is_null() {
            None
        } else {
            CStr::from_ptr(surrounding_text).to_str().ok()
        };

        let action = c.process_key(key_sym, k_str, has_ctrl_alt, s_text, cursor, anchor);

        let log_enabled = c.debug_log
            || std::env::var("CLAK_LOG")
                .map(|v| v != "0" && v != "false" && v != "off")
                .unwrap_or(false);
        if log_enabled {
            use std::io::Write;
            use std::os::unix::fs::MetadataExt;
            use std::os::unix::fs::OpenOptionsExt;

            let mut opts = std::fs::OpenOptions::new();
            opts.create(true)
                .append(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
            if let Ok(mut f) = opts.open("/tmp/clak.log") {
                if let Ok(meta) = f.metadata() {
                    if meta.uid() == unsafe { libc::getuid() } && meta.file_type().is_file() {
                        let act_name = match action.action_type {
                            0 => "FORWARD",
                            1 => "COMMIT",
                            2 => "REPLACE_SURR",
                            3 => "ADDR_BAR_FIX",
                            4 => "REPLACE",
                            _ => "UNKNOWN",
                        };
                        let commit = if action.commit_str.is_null() {
                            ""
                        } else {
                            CStr::from_ptr(action.commit_str).to_str().unwrap_or("")
                        };
                        let mut surr_preview = String::from("none");
                        if let Some(t) = s_text {
                            let preview: String = t.chars().take(25).collect();
                            surr_preview = format!("'{}'(c={},a={})", preview, cursor, anchor);
                        }
                        let _ = writeln!(
                            f,
                            "Key: '{}' (0x{:x}) | Surr: {} | Act: {} (del={}) -> Commit: '{}' | Raw: '{}' | Composed: '{}'",
                            k_str, key_sym, surr_preview, act_name, action.delete_count, commit, c.raw_buffer, c.last_composed
                        );
                    }
                }
            }
        }

        action
    }));

    match result {
        Ok(action) => action,
        Err(_) => ImeAction {
            action_type: ACTION_FORWARD,
            delete_count: 0,
            commit_str: std::ptr::null(),
            delete_str: std::ptr::null(),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn clak_observe_retype(
    old_raw: *const c_char,
    new_word: *const c_char,
) -> bool {
    if old_raw.is_null() || new_word.is_null() {
        return false;
    }
    let r = match CStr::from_ptr(old_raw).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let w = match CStr::from_ptr(new_word).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    crate::learned_typos::observe_retype(r, w).is_some()
}
