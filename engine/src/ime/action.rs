use std::os::raw::c_char;

#[repr(C)]
pub struct ImeAction {
    pub action_type: i32,
    pub delete_count: usize,
    pub commit_str: *const c_char,
    pub delete_str: *const c_char,
}

pub const ACTION_FORWARD: i32 = 0;
pub const ACTION_COMMIT: i32 = 1;
pub const ACTION_REPLACE_SURROUNDING: i32 = 2;
pub const ACTION_ADDRESS_BAR_FIX: i32 = 3;
pub const ACTION_REPLACE: i32 = 4;
pub const ACTION_UINPUT_REPLACE: i32 = 4;
pub const ACTION_AUTOCORRECT: i32 = 5;
