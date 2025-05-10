use embassy_rp::gpio::{AnyPin, Input, Level, Output, Pull};
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use heapless::Vec;
use picodox_proto::{KeyUpdate, MatrixLoc};

use crate::util::MutexType;

pub struct KeyMatrix<'d, const R: usize, const C: usize> {
    col_pins: [Output<'d>; C],
    row_pins: [Input<'d>; R],
    signal: &'d Signal<MutexType, KeyUpdate>,
    update_freq_ms: u32,
}

impl<'d, const R: usize, const C: usize> KeyMatrix<'d, R, C> {
    pub fn new(
        col_pins: [AnyPin; C],
        row_pins: [AnyPin; R],
        signal: &'d Signal<MutexType, KeyUpdate>,
        update_freq_ms: u32,
    ) -> Self {
        let col_pins = col_pins.map(|pin| Output::new(pin, Level::Low));
        let row_pins = row_pins.map(|pin| Input::new(pin, Pull::Down));

        KeyMatrix {
            col_pins,
            row_pins,
            signal,
            update_freq_ms,
        }
    }

    pub async fn run(mut self) -> ! {
        loop {
            // Create a report
            let mut code_vec = Vec::new();

            for (col, col_pin) in self.col_pins.iter_mut().enumerate() {
                col_pin.set_high();
                Timer::after_micros(20).await;
                for (row, row_pin) in self.row_pins.iter_mut().enumerate() {
                    if row_pin.is_high() {
                        // TODO: Ignore NKRO for now
                        let _ = code_vec.push(MatrixLoc::new(row, col));
                    }
                }
                col_pin.set_low();
            }

            let update = KeyUpdate::from_vec(code_vec);
            self.signal.signal(update);

            Timer::after_millis(self.update_freq_ms.into()).await;
        }
    }
}
