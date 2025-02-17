use heapless::Vec;
use picodox_proto::KeyUpdate;
use usbd_hid::descriptor::KeyboardReport;

use crate::{key_codes::*, key_hid::Keymap};

pub const NUM_ROWS: usize = 5;
pub const NUM_COLS: usize = 7;

const LEFT_KEY_MATRIX: [[Key; NUM_COLS]; NUM_ROWS] = [
    // K1-K7
    [KEY_NONE, KEY_5, KEY_4, KEY_3, KEY_2, KEY_1, KEY_GRAVE],
    // K8-K14
    [KEY_LEFTBRACE, KEY_T, KEY_R, KEY_E, KEY_W, KEY_Q, KEY_TAB],
    // K15-K21
    [KEY_PAGEUP, KEY_G, KEY_F, KEY_D, KEY_S, KEY_A, KEY_ESC],
    // K22-K28
    [
        KEY_PAGEDOWN,
        KEY_B,
        KEY_V,
        KEY_C,
        KEY_X,
        KEY_Z,
        KEY_NONE,
    ],
    // K29-K35
    [
        KEY_MOD_LSHIFT,
        KEY_BACKSPACE,
        KEY_MOD_LCTRL,
        KEY_MOD_LALT,
        KEY_NONE,
        KEY_NONE,
        KEY_MOD_LMETA,
    ],
];

const RIGHT_KEY_MATRIX: [[Key; NUM_COLS]; NUM_ROWS] = [
    // K1-K7
    [KEY_NONE, KEY_6, KEY_7, KEY_8, KEY_9, KEY_0, KEY_EQUAL],
    // K8-K14
    [KEY_RIGHTBRACE, KEY_Y, KEY_U, KEY_I, KEY_O, KEY_P, KEY_MINUS],
    // K15-K21
    [KEY_END, KEY_H, KEY_J, KEY_K, KEY_L, KEY_SEMICOLON, KEY_APOSTROPHE],
    // K22-K28
    [KEY_HOME, KEY_N, KEY_M, KEY_COMMA, KEY_DOT, KEY_SLASH, KEY_NONE],
    // K29-K35
    [KEY_ENTER, KEY_SPACE, KEY_NONE, KEY_LEFT, KEY_DOWN, KEY_UP, KEY_RIGHT],
];

pub struct BasicKeymap {
}

impl Keymap for BasicKeymap {
    fn get_report(&mut self, left: &KeyUpdate, right: &KeyUpdate) -> KeyboardReport {
        let mut code_vec: Vec<u8, 6> = Vec::new();
        let mut modifier = 0u8;

        for &lc in &left.0 {
            let code = LEFT_KEY_MATRIX[lc.row()][lc.col()];
            match code {
                Key::Mod(KeyMod(m)) => modifier |= m,
                Key::Code(KeyCode(c)) => {
                    let _ = code_vec.push(c);
                }
            }
        }

        for &rc in &right.0 {
            let code = RIGHT_KEY_MATRIX[rc.row()][rc.col()];
            match code {
                Key::Mod(KeyMod(m)) => modifier |= m,
                Key::Code(KeyCode(c)) => {
                    let _ = code_vec.push(c);
                }
            }
        }

        let mut keycodes = [0u8; 6];
        keycodes[..code_vec.len()].copy_from_slice(&code_vec);

        KeyboardReport {
            keycodes,
            leds: 0,
            modifier,
            reserved: 0,
        }
    }
}
