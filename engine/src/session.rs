use crate::{ClakConfig, ClakContext, Method};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub const MAX_BUFFERED_KEYS: usize = 1024;

pub const TIMER_SAFETY: u32 = 1;
pub const TIMER_COMMIT: u32 = 2;
pub const TIMER_SURR_LAG: u32 = 3;

pub const CMD_FORWARD_KEY: i32 = 0;
pub const CMD_COMMIT_STRING: i32 = 1;
pub const CMD_DELETE_SURROUNDING: i32 = 2;
pub const CMD_EMIT_UINPUT_BACKSPACE: i32 = 3;
pub const CMD_START_TIMER: i32 = 4;
pub const CMD_CANCEL_TIMER: i32 = 5;
pub const CMD_FORWARD_BACKSPACE: i32 = 6;
pub const CMD_START_CUSTOM_DELETION: i32 = 7;

pub const DELETION_SURROUNDING: i32 = 0;
pub const DELETION_UINPUT: i32 = 1;
pub const DELETION_CUSTOM: i32 = 2;
pub const DELETION_FORWARD_BS: i32 = 3;
pub const DELETION_COMMITTED_DEL: i32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClakSessionCommand {
    pub cmd_type: i32,
    pub sym: u32,
    pub text: *const c_char,
    pub offset: i32,
    pub count: u32,
    pub post_delay_ms: u32,
    pub pre_delay_ms: u32,
    pub gap_ms: u32,
    pub hold_us: u32,
    pub timer_id: u32,
    pub delay_us: u64,
}

impl Default for ClakSessionCommand {
    fn default() -> Self {
        Self {
            cmd_type: CMD_FORWARD_KEY,
            sym: 0,
            text: std::ptr::null(),
            offset: 0,
            count: 0,
            post_delay_ms: 0,
            pre_delay_ms: 0,
            gap_ms: 0,
            hold_us: 0,
            timer_id: 0,
            delay_us: 0,
        }
    }
}

#[repr(C)]
pub struct ClakSessionCommandList {
    pub count: usize,
    pub commands: *const ClakSessionCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferedKey {
    pub sym: u32,
    pub key_str: String,
    pub has_ctrl_alt: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeletionReason {
    Uinput,
    ForwardBackspace,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Composing,
    Deleting {
        expected_backspaces: usize,
        received_backspaces: usize,
        real_delete_count: usize,
        pending_commit: String,
        reason: DeletionReason,
        fallback_seq: u64,
    },
    Replaying,
}

pub struct ClakSession {
    ctx: ClakContext,
    state: SessionState,
    key_buffer: Vec<BufferedKey>,
    fallback_seq: u64,
    commands_buf: Vec<ClakSessionCommand>,
    text_storage: Vec<CString>,
    pending_commit_cache: CString,
    last_autofix_delete: CString,
    last_autofix_commit: CString,
}

impl ClakSession {
    pub fn new(method: Method) -> Self {
        Self {
            ctx: ClakContext::new(method),
            state: SessionState::Idle,
            key_buffer: Vec::with_capacity(MAX_BUFFERED_KEYS),
            fallback_seq: 0,
            commands_buf: Vec::with_capacity(16),
            text_storage: Vec::with_capacity(16),
            pending_commit_cache: CString::default(),
            last_autofix_delete: CString::default(),
            last_autofix_commit: CString::default(),
        }
    }

    pub fn context(&self) -> &ClakContext {
        &self.ctx
    }

    pub fn context_mut(&mut self) -> &mut ClakContext {
        &mut self.ctx
    }

    pub fn apply_config(&mut self, cfg: &ClakConfig) {
        self.ctx.apply_config(cfg);
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn is_deleting(&self) -> bool {
        matches!(self.state, SessionState::Deleting { .. })
    }

    pub fn buffered_keys_count(&self) -> usize {
        self.key_buffer.len()
    }

    pub fn expected_backspaces(&self) -> usize {
        match self.state {
            SessionState::Deleting {
                expected_backspaces,
                ..
            } => expected_backspaces,
            _ => 0,
        }
    }

    pub fn current_backspace_count(&self) -> usize {
        match self.state {
            SessionState::Deleting {
                received_backspaces,
                ..
            } => received_backspaces,
            _ => 0,
        }
    }

    pub fn pending_commit(&self) -> &str {
        match &self.state {
            SessionState::Deleting { pending_commit, .. } => pending_commit.as_str(),
            _ => "",
        }
    }

    pub fn pending_commit_c_str(&self) -> *const c_char {
        self.pending_commit_cache.as_ptr()
    }

    pub fn last_autofix_delete(&self) -> *const c_char {
        self.last_autofix_delete.as_ptr()
    }

    pub fn last_autofix_commit(&self) -> *const c_char {
        self.last_autofix_commit.as_ptr()
    }

    pub fn reset(&mut self, _force: bool) {
        self.ctx.reset();
        self.key_buffer.clear();
        self.fallback_seq = self.fallback_seq.wrapping_add(1);
        self.state = SessionState::Idle;
        self.pending_commit_cache = CString::default();
        self.last_autofix_delete = CString::default();
        self.last_autofix_commit = CString::default();
        self.commands_buf.clear();
        self.text_storage.clear();
    }

    fn push_command(&mut self, cmd: ClakSessionCommand) {
        self.commands_buf.push(cmd);
    }

    fn push_commit(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let c_str = CString::new(text).unwrap_or_default();
        let ptr = c_str.as_ptr();
        self.text_storage.push(c_str);
        self.push_command(ClakSessionCommand {
            cmd_type: CMD_COMMIT_STRING,
            text: ptr,
            ..Default::default()
        });
    }

    fn push_forward(&mut self, sym: u32) {
        self.push_command(ClakSessionCommand {
            cmd_type: CMD_FORWARD_KEY,
            sym,
            ..Default::default()
        });
    }

    fn push_delete_surrounding(&mut self, offset: i32, count: u32) {
        self.push_command(ClakSessionCommand {
            cmd_type: CMD_DELETE_SURROUNDING,
            offset,
            count,
            ..Default::default()
        });
    }

    fn push_emit_uinput(
        &mut self,
        count: usize,
        post_delay_ms: u32,
        pre_delay_ms: u32,
        gap_ms: u32,
        hold_us: u32,
    ) {
        self.push_command(ClakSessionCommand {
            cmd_type: CMD_EMIT_UINPUT_BACKSPACE,
            count: count as u32,
            post_delay_ms,
            pre_delay_ms,
            gap_ms,
            hold_us,
            ..Default::default()
        });
    }

    fn push_start_timer(&mut self, timer_id: u32, delay_us: u64) {
        self.push_command(ClakSessionCommand {
            cmd_type: CMD_START_TIMER,
            timer_id,
            delay_us,
            ..Default::default()
        });
    }

    fn push_cancel_timer(&mut self, timer_id: u32) {
        self.push_command(ClakSessionCommand {
            cmd_type: CMD_CANCEL_TIMER,
            timer_id,
            ..Default::default()
        });
    }

    fn prepare_command_list(&self) -> ClakSessionCommandList {
        ClakSessionCommandList {
            count: self.commands_buf.len(),
            commands: if self.commands_buf.is_empty() {
                std::ptr::null()
            } else {
                self.commands_buf.as_ptr()
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_key(
        &mut self,
        sym: u32,
        key_str: &str,
        has_ctrl_alt: bool,
        surrounding_text: Option<&str>,
        cursor: usize,
        anchor: usize,
        deletion_mode: i32,
        autofill_extra: u32,
        uinput_post_delay_ms: u32,
        uinput_pre_delay_ms: u32,
        uinput_gap_ms: u32,
        uinput_hold_us: u32,
        safety_timeout_us: u64,
        _timestamp_us: u64,
    ) -> ClakSessionCommandList {
        self.commands_buf.clear();
        self.text_storage.clear();
        self.last_autofix_delete = CString::default();
        self.last_autofix_commit = CString::default();

        // 1. handle keystroke while waiting for async uinput or fallback deletion
        if let SessionState::Deleting {
            expected_backspaces,
            ref mut received_backspaces,
            ref pending_commit,
            reason,
            fallback_seq,
            ..
        } = self.state
        {
            // check if this is an incoming backspace (keycode 22 / XKB BackSpace 0xff08)
            let is_backspace = sym == 22 || sym == 0xff08;
            if is_backspace && reason == DeletionReason::Uinput {
                *received_backspaces += 1;
                if *received_backspaces >= expected_backspaces {
                    let commit = pending_commit.clone();
                    self.finish_deletion(&commit, fallback_seq);
                }
                return self.prepare_command_list();
            }

            // handle buffer overflow: finalize pending commit and replay to prevent key loss
            if self.key_buffer.len() >= MAX_BUFFERED_KEYS {
                let commit = pending_commit.clone();
                if !commit.is_empty() {
                    self.push_commit(&commit);
                }
                self.push_cancel_timer(TIMER_COMMIT);
                self.push_cancel_timer(TIMER_SAFETY);
                self.fallback_seq = self.fallback_seq.wrapping_add(1);
                self.state = SessionState::Idle;
                self.pending_commit_cache = CString::default();
                self.flush_buffer_as_forwards();
                self.push_forward(sym);
                return self.prepare_command_list();
            }

            // buffer printable/navigation key during deletion
            self.key_buffer.push(BufferedKey {
                sym,
                key_str: key_str.to_string(),
                has_ctrl_alt,
            });
            return self.prepare_command_list();
        }

        // 2. normal key processing via ClakContext
        let action =
            self.ctx
                .process_key(sym, key_str, has_ctrl_alt, surrounding_text, cursor, anchor);

        match action.action_type {
            crate::ime::ACTION_FORWARD => {
                self.push_forward(sym);
                if self.ctx.is_composing() {
                    self.state = SessionState::Composing;
                } else {
                    self.state = SessionState::Idle;
                }
            }
            crate::ime::ACTION_COMMIT => {
                let text = if !action.commit_str.is_null() {
                    unsafe { std::ffi::CStr::from_ptr(action.commit_str) }
                        .to_str()
                        .unwrap_or("")
                } else {
                    ""
                };
                self.push_commit(text);
                self.state = SessionState::Idle;
            }
            crate::ime::ACTION_REPLACE_SURROUNDING
            | crate::ime::ACTION_REPLACE
            | crate::ime::ACTION_ADDRESS_BAR_FIX
            | crate::ime::ACTION_AUTOCORRECT => {
                if action.action_type == crate::ime::ACTION_AUTOCORRECT
                    && !action.delete_str.is_null()
                    && !action.commit_str.is_null()
                {
                    unsafe {
                        self.last_autofix_delete = CStr::from_ptr(action.delete_str).to_owned();
                        self.last_autofix_commit = CStr::from_ptr(action.commit_str).to_owned();
                    }
                }
                let commit_text = if !action.commit_str.is_null() {
                    unsafe { std::ffi::CStr::from_ptr(action.commit_str) }
                        .to_str()
                        .unwrap_or("")
                } else {
                    ""
                };

                let real_bs = action.delete_count;
                if real_bs == 0 {
                    if !commit_text.is_empty() {
                        self.push_commit(commit_text);
                    }
                    self.state = if self.ctx.is_composing() {
                        SessionState::Composing
                    } else {
                        SessionState::Idle
                    };
                    return self.prepare_command_list();
                }

                let mode = if deletion_mode == DELETION_SURROUNDING
                    && surrounding_text.is_some()
                    && cursor < real_bs
                {
                    DELETION_UINPUT
                } else {
                    deletion_mode
                };

                match mode {
                    DELETION_UINPUT => {
                        let extra = if action.action_type == crate::ime::ACTION_ADDRESS_BAR_FIX {
                            1
                        } else {
                            autofill_extra as usize
                        };
                        let bs_to_send = real_bs + extra + 1;
                        self.fallback_seq = self.fallback_seq.wrapping_add(1);
                        let seq = self.fallback_seq;
                        self.pending_commit_cache = CString::new(commit_text).unwrap_or_default();
                        self.state = SessionState::Deleting {
                            expected_backspaces: bs_to_send,
                            received_backspaces: 0,
                            real_delete_count: real_bs,
                            pending_commit: commit_text.to_string(),
                            reason: DeletionReason::Uinput,
                            fallback_seq: seq,
                        };
                        self.push_emit_uinput(
                            bs_to_send,
                            uinput_post_delay_ms,
                            uinput_pre_delay_ms,
                            uinput_gap_ms,
                            uinput_hold_us,
                        );
                        let timeout = if safety_timeout_us > 0 {
                            safety_timeout_us
                        } else {
                            250_000
                        };
                        self.push_start_timer(TIMER_SAFETY, timeout);
                    }
                    DELETION_CUSTOM => {
                        self.fallback_seq = self.fallback_seq.wrapping_add(1);
                        let seq = self.fallback_seq;
                        self.pending_commit_cache = CString::new(commit_text).unwrap_or_default();
                        self.state = SessionState::Deleting {
                            expected_backspaces: real_bs,
                            received_backspaces: 0,
                            real_delete_count: real_bs,
                            pending_commit: commit_text.to_string(),
                            reason: DeletionReason::Custom,
                            fallback_seq: seq,
                        };
                        let c_str = CString::new(commit_text).unwrap_or_default();
                        let ptr = c_str.as_ptr();
                        self.text_storage.push(c_str);
                        self.push_command(ClakSessionCommand {
                            cmd_type: CMD_START_CUSTOM_DELETION,
                            count: real_bs as u32,
                            text: ptr,
                            ..Default::default()
                        });
                    }
                    DELETION_FORWARD_BS => {
                        self.fallback_seq = self.fallback_seq.wrapping_add(1);
                        let seq = self.fallback_seq;
                        self.pending_commit_cache = CString::new(commit_text).unwrap_or_default();
                        self.state = SessionState::Deleting {
                            expected_backspaces: 0,
                            received_backspaces: 0,
                            real_delete_count: real_bs,
                            pending_commit: commit_text.to_string(),
                            reason: DeletionReason::ForwardBackspace,
                            fallback_seq: seq,
                        };
                        self.push_command(ClakSessionCommand {
                            cmd_type: CMD_FORWARD_BACKSPACE,
                            count: real_bs as u32,
                            ..Default::default()
                        });
                        let timeout = if safety_timeout_us > 0 {
                            safety_timeout_us
                        } else {
                            20_000
                        };
                        self.push_start_timer(TIMER_COMMIT, timeout);
                    }
                    DELETION_COMMITTED_DEL => {
                        let mut del_str = String::with_capacity(real_bs + commit_text.len());
                        for _ in 0..real_bs {
                            del_str.push('\x7f');
                        }
                        del_str.push_str(commit_text);
                        self.push_commit(&del_str);
                        self.state = if self.ctx.is_composing() {
                            SessionState::Composing
                        } else {
                            SessionState::Idle
                        };
                    }
                    _ /* DELETION_SURROUNDING */ => {
                        self.push_delete_surrounding(-(real_bs as i32), real_bs as u32);
                        if !commit_text.is_empty() {
                            self.push_commit(commit_text);
                        }
                        self.state = if self.ctx.is_composing() {
                            SessionState::Composing
                        } else {
                            SessionState::Idle
                        };
                    }
                }
            }
            _ => {
                self.push_forward(sym);
            }
        }

        self.prepare_command_list()
    }

    pub fn switch_to_forward_fallback(&mut self, delay_us: u64) -> ClakSessionCommandList {
        self.commands_buf.clear();
        self.text_storage.clear();
        self.last_autofix_delete = CString::default();
        self.last_autofix_commit = CString::default();

        let count = if let SessionState::Deleting {
            ref mut reason,
            ref mut expected_backspaces,
            ref mut fallback_seq,
            real_delete_count,
            ..
        } = self.state
        {
            *reason = DeletionReason::ForwardBackspace;
            *expected_backspaces = 0;
            *fallback_seq = fallback_seq.wrapping_add(1);
            real_delete_count
        } else {
            return self.prepare_command_list();
        };

        self.push_cancel_timer(TIMER_SAFETY);
        self.push_command(ClakSessionCommand {
            cmd_type: CMD_FORWARD_BACKSPACE,
            count: count as u32,
            ..Default::default()
        });
        self.push_start_timer(TIMER_COMMIT, delay_us);

        self.prepare_command_list()
    }

    pub fn on_backspace_received(&mut self) -> bool {
        if let SessionState::Deleting {
            expected_backspaces,
            ref mut received_backspaces,
            reason: DeletionReason::Uinput,
            ..
        } = self.state
        {
            *received_backspaces += 1;
            return *received_backspaces >= expected_backspaces;
        }
        false
    }

    pub fn on_sentinel_received(&mut self, _timestamp_us: u64) -> ClakSessionCommandList {
        self.commands_buf.clear();
        self.text_storage.clear();

        if let SessionState::Deleting {
            ref pending_commit,
            fallback_seq,
            ..
        } = self.state
        {
            let commit = pending_commit.clone();
            self.finish_deletion(&commit, fallback_seq);
        }

        self.prepare_command_list()
    }

    pub fn on_timer_fired(&mut self, timer_id: u32, _timestamp_us: u64) -> ClakSessionCommandList {
        self.commands_buf.clear();
        self.text_storage.clear();

        if let SessionState::Deleting {
            ref pending_commit,
            fallback_seq,
            ..
        } = self.state
        {
            if timer_id == TIMER_SAFETY || timer_id == TIMER_COMMIT {
                let commit = pending_commit.clone();
                self.finish_deletion(&commit, fallback_seq);
            }
        }

        self.prepare_command_list()
    }

    fn finish_deletion(&mut self, pending_commit: &str, _fallback_seq: u64) {
        self.push_cancel_timer(TIMER_SAFETY);
        self.push_cancel_timer(TIMER_COMMIT);
        self.state = SessionState::Idle;
        self.pending_commit_cache = CString::default();
        self.replay_buffered_keys(pending_commit);
    }

    fn replay_buffered_keys(&mut self, initial_batch: &str) {
        let mut batch_commit = initial_batch.to_string();
        let keys: Vec<BufferedKey> = self.key_buffer.drain(..).collect();
        self.state = SessionState::Replaying;

        for key in keys {
            let action = self
                .ctx
                .process_key(key.sym, &key.key_str, key.has_ctrl_alt, None, 0, 0);

            let commit_str = if !action.commit_str.is_null() {
                unsafe { std::ffi::CStr::from_ptr(action.commit_str) }
                    .to_str()
                    .unwrap_or("")
            } else {
                ""
            };

            match action.action_type {
                crate::ime::ACTION_FORWARD => {
                    if !key.has_ctrl_alt && !key.key_str.is_empty() && key.sym < 0xff00 {
                        // batch consecutive raw printable characters to minimize wayland round-trips
                        batch_commit.push_str(&key.key_str);
                    } else {
                        if !batch_commit.is_empty() {
                            self.push_commit(&batch_commit);
                            batch_commit.clear();
                        }
                        self.push_forward(key.sym);
                    }
                }
                crate::ime::ACTION_COMMIT => {
                    batch_commit.push_str(commit_str);
                }
                crate::ime::ACTION_REPLACE_SURROUNDING
                | crate::ime::ACTION_REPLACE
                | crate::ime::ACTION_ADDRESS_BAR_FIX
                | crate::ime::ACTION_AUTOCORRECT => {
                    let real_bs = action.delete_count;
                    let avail = batch_commit.chars().count();
                    if avail >= real_bs {
                        // absorb deletions in memory by popping chars from batch_commit
                        let keep = avail - real_bs;
                        batch_commit = batch_commit.chars().take(keep).collect();
                        batch_commit.push_str(commit_str);
                    } else {
                        let remaining_bs = real_bs - avail;
                        batch_commit.clear();
                        self.push_delete_surrounding(-(remaining_bs as i32), remaining_bs as u32);
                        if !commit_str.is_empty() {
                            self.push_commit(commit_str);
                        }
                    }
                }
                _ => {
                    if !batch_commit.is_empty() {
                        self.push_commit(&batch_commit);
                        batch_commit.clear();
                    }
                    self.push_forward(key.sym);
                }
            }
        }

        if !batch_commit.is_empty() {
            self.push_commit(&batch_commit);
        }

        self.state = if self.ctx.is_composing() {
            SessionState::Composing
        } else {
            SessionState::Idle
        };
    }

    fn flush_buffer_as_forwards(&mut self) {
        let keys: Vec<BufferedKey> = self.key_buffer.drain(..).collect();
        for k in keys {
            self.push_forward(k.sym);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle_and_typing() {
        let mut session = ClakSession::new(Method::Telex);
        assert_eq!(*session.state(), SessionState::Idle);

        // type 't'
        let list = session.process_key(
            't' as u32,
            "t",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(list.count, 1);
        unsafe {
            assert_eq!((*list.commands).cmd_type, CMD_FORWARD_KEY);
            assert_eq!((*list.commands).sym, 't' as u32);
        }
        assert!(session.context().is_composing());

        // type 'i'
        let list = session.process_key(
            'i' as u32,
            "i",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(list.count, 1);
        unsafe {
            assert_eq!((*list.commands).cmd_type, CMD_FORWARD_KEY);
        }

        // type 'e'
        let list = session.process_key(
            'e' as u32,
            "e",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(list.count, 1);

        // type 'e' -> replace 'e' with 'ê'
        let list = session.process_key(
            'e' as u32,
            "e",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(list.count, 2);
        unsafe {
            assert_eq!((*list.commands.add(0)).cmd_type, CMD_DELETE_SURROUNDING);
            assert_eq!((*list.commands.add(1)).cmd_type, CMD_COMMIT_STRING);
            let s = std::ffi::CStr::from_ptr((*list.commands.add(1)).text)
                .to_str()
                .unwrap();
            assert_eq!(s, "ê");
        }
    }

    #[test]
    fn test_session_uinput_buffering_and_sentinel_recovery() {
        let mut session = ClakSession::new(Method::Telex);

        // type 'u'
        session.process_key(
            'u' as u32,
            "u",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );

        // type 'w' with uinput -> transform to 'ư' using uinput
        let list = session.process_key(
            'w' as u32,
            "w",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1000,
        );
        assert_eq!(list.count, 2);
        unsafe {
            assert_eq!((*list.commands.add(0)).cmd_type, CMD_EMIT_UINPUT_BACKSPACE);
            assert_eq!((*list.commands.add(0)).count, 2); // 1 real + 1 sentinel
            assert_eq!((*list.commands.add(1)).cmd_type, CMD_START_TIMER);
            assert_eq!((*list.commands.add(1)).timer_id, TIMER_SAFETY);
        }
        assert!(session.is_deleting());
        assert_eq!(session.expected_backspaces(), 2);
        assert_eq!(session.pending_commit(), "ư");

        // incoming key while deleting must be buffered
        let list2 = session.process_key(
            'a' as u32,
            "a",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1005,
        );
        assert_eq!(list2.count, 0);
        assert_eq!(session.buffered_keys_count(), 1);

        // sentinel received -> flushes pending commit and buffered keys
        let list3 = session.on_sentinel_received(1020);
        assert!(list3.count >= 2);
        assert_eq!(session.buffered_keys_count(), 0);
        assert!(!session.is_deleting());
    }

    #[test]
    fn test_session_buffer_overflow_finalizes_commit() {
        let mut session = ClakSession::new(Method::Telex);

        // type 'u'
        session.process_key(
            'u' as u32,
            "u",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );

        // type 'w' with uinput
        session.process_key(
            'w' as u32,
            "w",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1000,
        );
        assert!(session.is_deleting());

        // buffer 32 keys
        for _ in 0..MAX_BUFFERED_KEYS {
            let l = session.process_key(
                'a' as u32,
                "a",
                false,
                None,
                0,
                0,
                DELETION_UINPUT,
                0,
                10,
                5,
                2,
                100,
                250_000,
                1005,
            );
            assert_eq!(l.count, 0);
        }
        assert_eq!(session.buffered_keys_count(), MAX_BUFFERED_KEYS);

        // 33rd key triggers overflow protection
        let overflow_list = session.process_key(
            'z' as u32,
            "z",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1010,
        );
        // commit 'ư' + 2 cancel timers + 32 'a' forwards + 1 'z' forward
        assert_eq!(overflow_list.count, 1 + 2 + MAX_BUFFERED_KEYS + 1);
        assert!(!session.is_deleting());
        assert_eq!(session.buffered_keys_count(), 0);
    }

    #[test]
    fn test_session_batch_absorption_during_replay() {
        let mut session = ClakSession::new(Method::Telex);

        // type 'u'
        session.process_key(
            'u' as u32,
            "u",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );

        // type 'w' with uinput
        session.process_key(
            'w' as u32,
            "w",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1000,
        );
        assert!(session.is_deleting());

        // buffer 'o', 'n', 'g' while deleting
        session.process_key(
            'o' as u32,
            "o",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1005,
        );
        session.process_key(
            'n' as u32,
            "n",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1006,
        );
        session.process_key(
            'g' as u32,
            "g",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1007,
        );
        assert_eq!(session.buffered_keys_count(), 3);

        // sentinel received -> batched commit "ương"
        let list = session.on_sentinel_received(1020);
        assert_eq!(list.count, 3);
        unsafe {
            assert_eq!((*list.commands.add(0)).cmd_type, CMD_CANCEL_TIMER);
            assert_eq!((*list.commands.add(1)).cmd_type, CMD_CANCEL_TIMER);
            assert_eq!((*list.commands.add(2)).cmd_type, CMD_COMMIT_STRING);
            let s = std::ffi::CStr::from_ptr((*list.commands.add(2)).text)
                .to_str()
                .unwrap();
            assert_eq!(s, "ương");
        }
    }
}
