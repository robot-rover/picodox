use crate::key_codes::*;

pub const NUM_ROWS: usize = 5;
pub const NUM_COLS: usize = 7;

const LEFT_KEY_MATRIX_T: [[Key; NUM_COLS]; NUM_ROWS] = [
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
        KEY_MOD_LSHIFT,
    ],
    // K29-K35
    [
        KEY_DELETE,
        KEY_BACKSPACE,
        KEY_MOD_LCTRL,
        KEY_MOD_LALT,
        KEY_KPMINUS,
        KEY_KPPLUS,
        KEY_1,
    ],
];

pub const LEFT_KEY_MATRIX: [[Key; NUM_ROWS]; NUM_COLS] = const {
    let mut transpose: [[Key; NUM_ROWS]; NUM_COLS] = [[KEY_NONE; NUM_ROWS]; NUM_COLS];

    let mut row = 0;
    while row < NUM_ROWS {
        let mut col = 0;
        while col < NUM_COLS {
            transpose[col][row] = LEFT_KEY_MATRIX_T[row][col];
            col += 1;
        }
        row += 1;
    }

    transpose
};
