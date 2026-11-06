#!/usr/bin/env python3
import os
import sys
import time
import struct
import fcntl

UI_SET_EVBIT = 0x40045564
UI_SET_KEYBIT = 0x40045565
UI_DEV_SETUP = 0x405c5503
UI_DEV_CREATE = 0x5501
UI_DEV_DESTROY = 0x5502

EV_SYN = 0
EV_KEY = 1
SYN_REPORT = 0

KEY_BACKSPACE = 14
KEY_ENTER = 28
KEY_LEFTCTRL = 29
KEY_LEFTSHIFT = 42
KEY_SPACE = 57
KEY_COMMA = 51
KEY_DOT = 52

CHAR_TO_KEY = {
    'a': 30, 'b': 48, 'c': 46, 'd': 32, 'e': 18, 'f': 33, 'g': 34,
    'h': 35, 'i': 23, 'j': 36, 'k': 37, 'l': 38, 'm': 50, 'n': 49,
    'o': 24, 'p': 25, 'q': 16, 'r': 19, 's': 31, 't': 20, 'u': 22,
    'v': 47, 'w': 17, 'x': 45, 'y': 21, 'z': 44,
    '1': 2, '2': 3, '3': 4, '4': 5, '5': 6, '6': 7, '7': 8, '8': 9, '9': 10, '0': 11,
    ' ': KEY_SPACE, ',': KEY_COMMA, '.': KEY_DOT, '\n': KEY_ENTER
}

class UinputVirtualKeyboard:
    def __init__(self, settle_sec=0.45):
        self.fd = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
        fcntl.ioctl(self.fd, UI_SET_EVBIT, EV_KEY)
        for code in set(CHAR_TO_KEY.values()) | {KEY_BACKSPACE, KEY_LEFTCTRL, KEY_LEFTSHIFT}:
            fcntl.ioctl(self.fd, UI_SET_KEYBIT, code)
        setup_data = struct.pack('HHHH80sI', 0x03, 0x1234, 0x5679, 1, b'clak-test-kbd', 0)
        fcntl.ioctl(self.fd, UI_DEV_SETUP, setup_data)
        fcntl.ioctl(self.fd, UI_DEV_CREATE)
        if settle_sec > 0:
            time.sleep(settle_sec)

    def _emit_key(self, code, val):
        ev = struct.pack('llHHI', 0, 0, EV_KEY, code, val)
        syn = struct.pack('llHHI', 0, 0, EV_SYN, SYN_REPORT, 0)
        os.write(self.fd, ev + syn)

    def tap_key(self, code, hold_us=1000):
        self._emit_key(code, 1)
        if hold_us > 0:
            time.sleep(hold_us / 1000000.0)
        self._emit_key(code, 0)

    def type_text(self, text, gap_ms=10.0, hold_us=1000):
        gap_s = max(0.0, (gap_ms * 1000.0 - hold_us) / 1000000.0)
        for ch in text:
            lower = ch.lower()
            code = CHAR_TO_KEY.get(lower)
            if code is None:
                continue
            is_upper = ch.isupper()
            if is_upper:
                self._emit_key(KEY_LEFTSHIFT, 1)
            self.tap_key(code, hold_us=hold_us)
            if is_upper:
                self._emit_key(KEY_LEFTSHIFT, 0)
            if gap_s > 0:
                time.sleep(gap_s)

    def send_ctrl_d(self, hold_us=2000):
        self._emit_key(KEY_LEFTCTRL, 1)
        time.sleep(0.002)
        self.tap_key(CHAR_TO_KEY['d'], hold_us=hold_us)
        time.sleep(0.002)
        self._emit_key(KEY_LEFTCTRL, 0)

    def close(self):
        if self.fd is not None:
            try:
                fcntl.ioctl(self.fd, UI_DEV_DESTROY)
                os.close(self.fd)
            except Exception:
                pass
            self.fd = None
