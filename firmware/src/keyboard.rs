use defmt::{info, warn};
use embassy_futures::join::join;
use embassy_rp::gpio::{Input, Pin, Pull};
use embassy_usb::{class::hid::{Config, HidReader, HidReaderWriter, HidWriter, ReportId, RequestHandler, State}, control::OutResponse, driver::Driver, Builder};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor as _};

pub struct KeyboardIf<'d, D: Driver<'d>> {
    reader: HidReader<'d, D, 1>,
    writer: HidWriter<'d, D, 8>,
    pin: Input<'d>,
}

impl<'d, D: Driver<'d>> KeyboardIf<'d, D> {
    pub fn new(builder: &mut Builder<'d, D>, state: &'d mut State<'d>, pin: impl Pin) -> Self {
        let config = Config {
            report_descriptor: KeyboardReport::desc(),
            request_handler: None,
            poll_ms: 60,
            max_packet_size: 64,
        };
        let hid = HidReaderWriter::<_, 1, 8>::new(builder, state, config);
        let (reader, writer) = hid.split();

        let mut sig_pin = Input::new(pin, Pull::Up);
        sig_pin.set_schmitt(true);

        KeyboardIf {
            reader,
            writer,
            pin: sig_pin,
        }
    }

    pub async fn run(mut self) {
        let in_fut = async {
            loop {
                info!("Waiting for HIGH on pin 16");
                self.pin.wait_for_low().await;
                info!("LOW DETECTED");
                // Create a report with the A key pressed. (no shift modifier)
                let report = KeyboardReport {
                    keycodes: [4, 0, 0, 0, 0, 0],
                    leds: 0,
                    modifier: 0,
                    reserved: 0,
                };
                // Send the report.
                match self.writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send report: {:?}", e),
                };
                self.pin.wait_for_high().await;
                info!("HIGH DETECTED");
                let report = KeyboardReport {
                    keycodes: [0, 0, 0, 0, 0, 0],
                    leds: 0,
                    modifier: 0,
                    reserved: 0,
                };
                match self.writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send report: {:?}", e),
                };
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
