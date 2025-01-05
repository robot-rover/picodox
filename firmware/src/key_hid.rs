use defmt::{info, warn};
use embassy_futures::join::join;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embassy_usb::{
    class::hid::{Config, HidReader, HidReaderWriter, HidWriter, ReportId, RequestHandler, State},
    control::OutResponse,
    driver::Driver,
    Builder,
};
use picodox_proto::KeyUpdate;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor as _};

use crate::util::MutexType;

pub trait Keymap {
    fn get_report(&mut self, left: &KeyUpdate, right: &KeyUpdate) -> KeyboardReport;
}

pub struct KeyboardIf<'d, D: Driver<'d>, K: Keymap> {
    reader: HidReader<'d, D, 1>,
    writer: HidWriter<'d, D, 8>,
    left_signal: &'d Signal<MutexType, KeyUpdate>,
    right_signal: &'d Signal<MutexType, KeyUpdate>,
    update_freq_ms: u32,
    keymap: K,
}

impl<'d, D: Driver<'d>, K: Keymap> KeyboardIf<'d, D, K> {
    pub fn new(
        builder: &mut Builder<'d, D>,
        state: &'d mut State<'d>,
        left_signal: &'d Signal<MutexType, KeyUpdate>,
        right_signal: &'d Signal<MutexType, KeyUpdate>,
        update_freq_ms: u32,
        keymap: K,
    ) -> Self {
        let config = Config {
            report_descriptor: KeyboardReport::desc(),
            request_handler: None,
            poll_ms: 60,
            max_packet_size: 64,
        };
        let hid = HidReaderWriter::<_, 1, 8>::new(builder, state, config);
        let (reader, writer) = hid.split();

        KeyboardIf {
            reader,
            writer,
            left_signal,
            right_signal,
            update_freq_ms,
            keymap,
        }
    }

    pub async fn run(mut self) {
        let in_fut = async {
            let mut left = KeyUpdate::no_keys();
            let mut right = KeyUpdate::no_keys();

            loop {
                if let Some(new_left) = self.left_signal.try_take() {
                    info!("Left Update: {}", new_left.0.len());
                    left = new_left;
                }

                if let Some(new_right) = self.right_signal.try_take() {
                    info!("Right Update: {}", new_right.0.len());
                    right = new_right;
                }

                let report = self.keymap.get_report(&left, &right);

                match self.writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send report: {:?}", e),
                };

                Timer::after_millis(self.update_freq_ms.into()).await;
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
