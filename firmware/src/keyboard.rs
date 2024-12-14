use defmt::{info, warn};
use embassy_futures::join::join;
use embassy_rp::gpio::{AnyPin, Input, Level, Output, Pull};
use embassy_time::Timer;
use embassy_usb::{
    class::hid::{Config, HidReader, HidReaderWriter, HidWriter, ReportId, RequestHandler, State},
    control::OutResponse,
    driver::Driver,
    Builder,
};
use heapless::Vec;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor as _};

use crate::{
    key_codes::{Key, KeyCode, KeyMod},
    key_matrix,
};

pub struct KeyboardIf<'d, D: Driver<'d>, const R: usize, const C: usize> {
    reader: HidReader<'d, D, 1>,
    writer: HidWriter<'d, D, 8>,
    col_pins: [Output<'d>; C],
    row_pins: [Input<'d>; R],
}

impl<'d, D: Driver<'d>, const R: usize, const C: usize> KeyboardIf<'d, D, R, C> {
    pub fn new(
        builder: &mut Builder<'d, D>,
        state: &'d mut State<'d>,
        col_pins: [AnyPin; C],
        row_pins: [AnyPin; R],
    ) -> Self {
        let config = Config {
            report_descriptor: KeyboardReport::desc(),
            request_handler: None,
            poll_ms: 60,
            max_packet_size: 64,
        };
        let hid = HidReaderWriter::<_, 1, 8>::new(builder, state, config);
        let (reader, writer) = hid.split();

        let col_pins = col_pins.map(|pin| Output::new(pin, Level::Low));
        let row_pins = row_pins.map(|pin| Input::new(pin, Pull::Down));

        KeyboardIf {
            reader,
            writer,
            col_pins,
            row_pins,
        }
    }

    pub async fn run(mut self) {
        let in_fut = async {
            loop {
                info!("Starting Scan");
                // Create a report
                let mut code_vec: Vec<u8, 6> = Vec::new();
                let mut modifier = 0u8;
                for (col, col_pin) in self.col_pins.iter_mut().enumerate() {
                    col_pin.set_high();
                    for (row, row_pin) in self.row_pins.iter_mut().enumerate() {
                        if row_pin.is_high() {
                            match key_matrix::LEFT_KEY_MATRIX[col][row] {
                                Key::Mod(KeyMod(byte)) => modifier |= byte,
                                Key::Code(KeyCode(byte)) => {
                                    // Ignore ROVR Overflow for now
                                    let _ = code_vec.push(byte);
                                }
                            }
                        }
                    }
                    col_pin.set_low();
                }

                let mut keycodes = [0u8; 6];
                keycodes[..code_vec.len()].copy_from_slice(&code_vec);

                let report = KeyboardReport {
                    keycodes,
                    leds: 0,
                    modifier,
                    reserved: 0,
                };
                match self.writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send report: {:?}", e),
                };

                Timer::after_millis(30).await;
            }
        };

        let out_fut = async {
            let mut request_handler = MyRequestHandler;
            self.reader.run(false, &mut request_handler).await;
        };
        join(in_fut, out_fut).await;
    }
}

struct MyRequestHandler;

impl RequestHandler for MyRequestHandler {
    fn get_report(&mut self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {:?}", id);
        None
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {=[u8]}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&mut self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {:?}", id);
        None
    }
}
