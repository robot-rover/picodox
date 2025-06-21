#![allow(dead_code)]

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Mod(KeyMod),
    Code(KeyCode),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct KeyMod(pub u8);

const fn kmod(key_mod: u8) -> Key {
    Key::Mod(KeyMod(key_mod))
}

pub const KEY_MOD_LCTRL: Key = kmod(0x01);
pub const KEY_MOD_LSHIFT: Key = kmod(0x02);
pub const KEY_MOD_LALT: Key = kmod(0x04);
pub const KEY_MOD_LMETA: Key = kmod(0x08);
pub const KEY_MOD_RCTRL: Key = kmod(0x10);
pub const KEY_MOD_RSHIFT: Key = kmod(0x20);
pub const KEY_MOD_RALT: Key = kmod(0x40);
pub const KEY_MOD_RMETA: Key = kmod(0x80);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct KeyCode(pub u8);

const fn kcode(key_code: u8) -> Key {
    Key::Code(KeyCode(key_code))
}

/// No key pressed
pub const KEY_NONE: Key = kcode(0x00);
///  Keyboard Error Roll Over - used for all slots if too many keys are pressed ("Phantom key")
pub const KEY_ERR_OVF: Key = kcode(0x01);
///  Keyboard POST Fail
pub const KEY_POST_FILE: Key = kcode(0x02);
///  Keyboard Error Undefined
pub const KEY_ERR: Key = kcode(0x03);
/// Keyboard a and A
pub const KEY_A: Key = kcode(0x04);
/// Keyboard b and B
pub const KEY_B: Key = kcode(0x05);
/// Keyboard c and C
pub const KEY_C: Key = kcode(0x06);
/// Keyboard d and D
pub const KEY_D: Key = kcode(0x07);
/// Keyboard e and E
pub const KEY_E: Key = kcode(0x08);
/// Keyboard f and F
pub const KEY_F: Key = kcode(0x09);
/// Keyboard g and G
pub const KEY_G: Key = kcode(0x0a);
/// Keyboard h and H
pub const KEY_H: Key = kcode(0x0b);
/// Keyboard i and I
pub const KEY_I: Key = kcode(0x0c);
/// Keyboard j and J
pub const KEY_J: Key = kcode(0x0d);
/// Keyboard k and K
pub const KEY_K: Key = kcode(0x0e);
/// Keyboard l and L
pub const KEY_L: Key = kcode(0x0f);
/// Keyboard m and M
pub const KEY_M: Key = kcode(0x10);
/// Keyboard n and N
pub const KEY_N: Key = kcode(0x11);
/// Keyboard o and O
pub const KEY_O: Key = kcode(0x12);
/// Keyboard p and P
pub const KEY_P: Key = kcode(0x13);
/// Keyboard q and Q
pub const KEY_Q: Key = kcode(0x14);
/// Keyboard r and R
pub const KEY_R: Key = kcode(0x15);
/// Keyboard s and S
pub const KEY_S: Key = kcode(0x16);
/// Keyboard t and T
pub const KEY_T: Key = kcode(0x17);
/// Keyboard u and U
pub const KEY_U: Key = kcode(0x18);
/// Keyboard v and V
pub const KEY_V: Key = kcode(0x19);
/// Keyboard w and W
pub const KEY_W: Key = kcode(0x1a);
/// Keyboard x and X
pub const KEY_X: Key = kcode(0x1b);
/// Keyboard y and Y
pub const KEY_Y: Key = kcode(0x1c);
/// Keyboard z and Z
pub const KEY_Z: Key = kcode(0x1d);

/// Keyboard 1 and !
pub const KEY_1: Key = kcode(0x1e);
/// Keyboard 2 and @
pub const KEY_2: Key = kcode(0x1f);
/// Keyboard 3 and #
pub const KEY_3: Key = kcode(0x20);
/// Keyboard 4 and $
pub const KEY_4: Key = kcode(0x21);
/// Keyboard 5 and %
pub const KEY_5: Key = kcode(0x22);
/// Keyboard 6 and ^
pub const KEY_6: Key = kcode(0x23);
/// Keyboard 7 and &
pub const KEY_7: Key = kcode(0x24);
/// Keyboard 8 and *
pub const KEY_8: Key = kcode(0x25);
/// Keyboard 9 and (
pub const KEY_9: Key = kcode(0x26);
/// Keyboard 0 and )
pub const KEY_0: Key = kcode(0x27);

/// Keyboard Return (ENTER)
pub const KEY_ENTER: Key = kcode(0x28);
/// Keyboard ESCAPE
pub const KEY_ESC: Key = kcode(0x29);
/// Keyboard DELETE (Backspace)
pub const KEY_BACKSPACE: Key = kcode(0x2a);
/// Keyboard Tab
pub const KEY_TAB: Key = kcode(0x2b);
/// Keyboard Spacebar
pub const KEY_SPACE: Key = kcode(0x2c);
/// Keyboard - and _
pub const KEY_MINUS: Key = kcode(0x2d);
/// Keyboard = and +
pub const KEY_EQUAL: Key = kcode(0x2e);
/// Keyboard [ and {
pub const KEY_LEFTBRACE: Key = kcode(0x2f);
/// Keyboard ] and }
pub const KEY_RIGHTBRACE: Key = kcode(0x30);
/// Keyboard \ and |
pub const KEY_BACKSLASH: Key = kcode(0x31);
/// Keyboard Non-US # and ~
pub const KEY_HASHTILDE: Key = kcode(0x32);
/// Keyboard ; and :
pub const KEY_SEMICOLON: Key = kcode(0x33);
/// Keyboard ' and "
pub const KEY_APOSTROPHE: Key = kcode(0x34);
/// Keyboard ` and ~
pub const KEY_GRAVE: Key = kcode(0x35);
/// Keyboard , and <
pub const KEY_COMMA: Key = kcode(0x36);
/// Keyboard . and >
pub const KEY_DOT: Key = kcode(0x37);
/// Keyboard / and ?
pub const KEY_SLASH: Key = kcode(0x38);
/// Keyboard Caps Lock
pub const KEY_CAPSLOCK: Key = kcode(0x39);

/// Keyboard F1
pub const KEY_F1: Key = kcode(0x3a);
/// Keyboard F2
pub const KEY_F2: Key = kcode(0x3b);
/// Keyboard F3
pub const KEY_F3: Key = kcode(0x3c);
/// Keyboard F4
pub const KEY_F4: Key = kcode(0x3d);
/// Keyboard F5
pub const KEY_F5: Key = kcode(0x3e);
/// Keyboard F6
pub const KEY_F6: Key = kcode(0x3f);
/// Keyboard F7
pub const KEY_F7: Key = kcode(0x40);
/// Keyboard F8
pub const KEY_F8: Key = kcode(0x41);
/// Keyboard F9
pub const KEY_F9: Key = kcode(0x42);
/// Keyboard F10
pub const KEY_F10: Key = kcode(0x43);
/// Keyboard F11
pub const KEY_F11: Key = kcode(0x44);
/// Keyboard F12
pub const KEY_F12: Key = kcode(0x45);

/// Keyboard Print Screen
pub const KEY_SYSRQ: Key = kcode(0x46);
/// Keyboard Scroll Lock
pub const KEY_SCROLLLOCK: Key = kcode(0x47);
/// Keyboard Pause
pub const KEY_PAUSE: Key = kcode(0x48);
/// Keyboard Insert
pub const KEY_INSERT: Key = kcode(0x49);
/// Keyboard Home
pub const KEY_HOME: Key = kcode(0x4a);
/// Keyboard Page Up
pub const KEY_PAGEUP: Key = kcode(0x4b);
/// Keyboard Delete Forward
pub const KEY_DELETE: Key = kcode(0x4c);
/// Keyboard End
pub const KEY_END: Key = kcode(0x4d);
/// Keyboard Page Down
pub const KEY_PAGEDOWN: Key = kcode(0x4e);
/// Keyboard Right Arrow
pub const KEY_RIGHT: Key = kcode(0x4f);
/// Keyboard Left Arrow
pub const KEY_LEFT: Key = kcode(0x50);
/// Keyboard Down Arrow
pub const KEY_DOWN: Key = kcode(0x51);
/// Keyboard Up Arrow
pub const KEY_UP: Key = kcode(0x52);

/// Keyboard Num Lock and Clear
pub const KEY_NUMLOCK: Key = kcode(0x53);
/// Keypad /
pub const KEY_KPSLASH: Key = kcode(0x54);
/// Keypad *
pub const KEY_KPASTERISK: Key = kcode(0x55);
/// Keypad -
pub const KEY_KPMINUS: Key = kcode(0x56);
/// Keypad +
pub const KEY_KPPLUS: Key = kcode(0x57);
/// Keypad ENTER
pub const KEY_KPENTER: Key = kcode(0x58);
/// Keypad 1 and End
pub const KEY_KP1: Key = kcode(0x59);
/// Keypad 2 and Down Arrow
pub const KEY_KP2: Key = kcode(0x5a);
/// Keypad 3 and PageDn
pub const KEY_KP3: Key = kcode(0x5b);
/// Keypad 4 and Left Arrow
pub const KEY_KP4: Key = kcode(0x5c);
/// Keypad 5
pub const KEY_KP5: Key = kcode(0x5d);
/// Keypad 6 and Right Arrow
pub const KEY_KP6: Key = kcode(0x5e);
/// Keypad 7 and Home
pub const KEY_KP7: Key = kcode(0x5f);
/// Keypad 8 and Up Arrow
pub const KEY_KP8: Key = kcode(0x60);
/// Keypad 9 and Page Up
pub const KEY_KP9: Key = kcode(0x61);
/// Keypad 0 and Insert
pub const KEY_KP0: Key = kcode(0x62);
/// Keypad . and Delete
pub const KEY_KPDOT: Key = kcode(0x63);

/// Keyboard Non-US \ and |
pub const KEY_102ND: Key = kcode(0x64);
/// Keyboard Application
pub const KEY_COMPOSE: Key = kcode(0x65);
/// Keyboard Power
pub const KEY_POWER: Key = kcode(0x66);
/// Keypad =
pub const KEY_KPEQUAL: Key = kcode(0x67);

/// Keyboard F13
pub const KEY_F13: Key = kcode(0x68);
/// Keyboard F14
pub const KEY_F14: Key = kcode(0x69);
/// Keyboard F15
pub const KEY_F15: Key = kcode(0x6a);
/// Keyboard F16
pub const KEY_F16: Key = kcode(0x6b);
/// Keyboard F17
pub const KEY_F17: Key = kcode(0x6c);
/// Keyboard F18
pub const KEY_F18: Key = kcode(0x6d);
/// Keyboard F19
pub const KEY_F19: Key = kcode(0x6e);
/// Keyboard F20
pub const KEY_F20: Key = kcode(0x6f);
/// Keyboard F21
pub const KEY_F21: Key = kcode(0x70);
/// Keyboard F22
pub const KEY_F22: Key = kcode(0x71);
/// Keyboard F23
pub const KEY_F23: Key = kcode(0x72);
/// Keyboard F24
pub const KEY_F24: Key = kcode(0x73);

/// Keyboard Execute
pub const KEY_OPEN: Key = kcode(0x74);
/// Keyboard Help
pub const KEY_HELP: Key = kcode(0x75);
/// Keyboard Menu
pub const KEY_PROPS: Key = kcode(0x76);
/// Keyboard Select
pub const KEY_FRONT: Key = kcode(0x77);
/// Keyboard Stop
pub const KEY_STOP: Key = kcode(0x78);
/// Keyboard Again
pub const KEY_AGAIN: Key = kcode(0x79);
/// Keyboard Undo
pub const KEY_UNDO: Key = kcode(0x7a);
/// Keyboard Cut
pub const KEY_CUT: Key = kcode(0x7b);
/// Keyboard Copy
pub const KEY_COPY: Key = kcode(0x7c);
/// Keyboard Paste
pub const KEY_PASTE: Key = kcode(0x7d);
/// Keyboard Find
pub const KEY_FIND: Key = kcode(0x7e);
/// Keyboard Mute
pub const KEY_MUTE: Key = kcode(0x7f);
/// Keyboard Volume Up
pub const KEY_VOLUMEUP: Key = kcode(0x80);
/// Keyboard Volume Down
pub const KEY_VOLUMEDOWN: Key = kcode(0x81);
