use defmt::info;
use heapless::Vec;
use picodox_proto::{KeyState, NUM_KEYS};
use usbd_hid::descriptor::KeyboardReport;

use crate::{key_codes::*, key_hid::Keymap};

const fn l(idx: usize) -> usize {
    idx - 1
}

const fn r(idx: usize) -> usize {
    NUM_KEYS + idx - 1
}

const fn from_pairs(pairs: &[(usize, Key)]) -> [Key; 2 * NUM_KEYS] {
    let mut result = [KEY_NONE; 2 * NUM_KEYS];
    let mut arr_idx = 0;
    while arr_idx < pairs.len() {
        let (idx, code) = pairs[arr_idx];
        result[idx] = code;

        arr_idx += 1;
    }
    result
}

const KEY_MATRIX: [Key; 2 * NUM_KEYS] = [
    // -- LEFT Side --
    // K1-K7
    KEY_NONE,
    KEY_5,
    KEY_4,
    KEY_3,
    KEY_2,
    KEY_1,
    KEY_GRAVE,
    // K8-K14
    KEY_LEFTBRACE,
    KEY_T,
    KEY_R,
    KEY_E,
    KEY_W,
    KEY_Q,
    KEY_TAB,
    // K15-K21
    KEY_PAGEUP,
    KEY_G,
    KEY_F,
    KEY_D,
    KEY_S,
    KEY_A,
    KEY_BACKSPACE,
    // K22-K28
    KEY_PAGEDOWN,
    KEY_B,
    KEY_V,
    KEY_C,
    KEY_X,
    KEY_Z,
    KEY_NONE,
    // K29-K35
    KEY_ESC,
    KEY_MOD_LSHIFT,
    KEY_MOD_LCTRL,
    KEY_MOD_LALT,
    KEY_BACKSLASH,
    KEY_DELETE,
    KEY_MOD_LMETA,
    // -- Right Side --
    // K1-K7
    KEY_NONE,
    KEY_6,
    KEY_7,
    KEY_8,
    KEY_9,
    KEY_0,
    KEY_EQUAL,
    // K8-K14
    KEY_RIGHTBRACE,
    KEY_Y,
    KEY_U,
    KEY_I,
    KEY_O,
    KEY_P,
    KEY_MINUS,
    // K15-K21
    KEY_END,
    KEY_H,
    KEY_J,
    KEY_K,
    KEY_L,
    KEY_SEMICOLON,
    KEY_APOSTROPHE,
    // K22-K28
    KEY_HOME,
    KEY_N,
    KEY_M,
    KEY_COMMA,
    KEY_DOT,
    KEY_SLASH,
    KEY_NONE,
    // K29-K35
    KEY_ENTER,
    KEY_SPACE,
    KEY_NONE,
    KEY_LEFT,
    KEY_DOWN,
    KEY_UP,
    KEY_RIGHT,
];

const NAV_MATRIX: [Key; 2 * NUM_KEYS] = from_pairs(&[
    (r(16), KEY_LEFT),
    (r(17), KEY_DOWN),
    (r(18), KEY_UP),
    (r(19), KEY_RIGHT),
]);

#[derive(Default)]
pub struct BasicKeymap {
    last_lparen: bool,
    last_rparen: bool,
}

impl Keymap for BasicKeymap {
    fn get_report(&mut self, state: &KeyState) -> KeyboardReport {
        let mut code_vec: Vec<u8, 6> = Vec::new();
        let mut modifier = 0u8;

        let nav_pressed = state.0[r(31)];
        info!("Nav Pressed: {}", nav_pressed);

        let matrix = if nav_pressed {
            &NAV_MATRIX
        } else {
            &KEY_MATRIX
        };

        for (key, code) in state.0.iter().cloned().zip(matrix.iter().cloned()) {
            if !key || code == KEY_NONE {
                continue;
            };
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
