use core::fmt::Write as _;

use heapless::String;
use rp_pico::hal::{fugit::{Duration, TimerInstantU64}, Timer};
use usb_device::{bus::{UsbBus, UsbBusAllocator}, device::UsbDevice};
// USB Communications Class Device support
use usbd_serial::SerialPort;

const CLOCK_FREQ: u32 = 1_000_000;

pub struct SerialIf<'a, B>
where B: UsbBus {
    port: SerialPort<'a, B>,
    timer: Timer,
    last_hello: TimerInstantU64<CLOCK_FREQ>,
}

impl<'a, B: UsbBus> SerialIf<'a, B> {
    pub fn setup<'alloc: 'a>(usb_bus: &'alloc UsbBusAllocator<B>, timer: Timer) -> Self {
        SerialIf {
            port: SerialPort::new(&usb_bus),
            timer,
            last_hello: TimerInstantU64::from_ticks(0),
        }
    }

    pub fn poll(&mut self, usb_dev: &mut UsbDevice<B>) {
        let now = self.timer.get_counter();
        let duration = now.checked_duration_since(self.last_hello)
            .unwrap_or(Duration::<u64, 1, CLOCK_FREQ>::from_ticks(0));
        // A welcome message at the beginning
        if duration.ticks() >= 2_000_000 {
            self.last_hello = now;
            let _ = self.port.write(b"Hello, World!\r\n");

            let time = self.timer.get_counter().ticks();
            let mut text: String<64> = String::new();
            writeln!(&mut text, "Current timer ticks: {}\r\n", time).unwrap();

            // This only works reliably because the number of bytes written to
            // the serial port is smaller than the buffers available to the USB
            // peripheral. In general, the return value should be handled, so that
            // bytes not transferred yet don't get lost.
            let _ = self.port.write(text.as_bytes());
        }

        // Check for new data
        if usb_dev.poll(&mut [&mut self.port]) {
            let mut buf = [0u8; 64];
            match self.port.read(&mut buf) {
                Err(_e) => {
                    // Do nothing
                }
                Ok(0) => {
                    // Do nothing
                }
                Ok(count) => {
                    // Convert to upper case
                    //buf.iter_mut().take(count).for_each(|b| {
                    //    b.make_ascii_uppercase();
                    //});
                    // Send back to the host
                    let mut wr_ptr = &buf[..count];
                    while !wr_ptr.is_empty() {
                        match self.port.write(wr_ptr) {
                            Ok(len) => wr_ptr = &wr_ptr[len..],
                            // On error, just drop unwritten data.
                            // One possible error is Err(WouldBlock), meaning the USB
                            // write buffer is full.
                            Err(_) => break,
                        };
                    }
                }
            }
        }
    }
}
