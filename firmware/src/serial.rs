use core::fmt::Write as _;

use circular_buffer::CircularBuffer;
use heapless::String;
use rp_pico::hal::Timer;
use usb_device::{bus::{UsbBus, UsbBusAllocator}, device::UsbDevice};
// USB Communications Class Device support
use usbd_serial::SerialPort;

pub struct SerialIf<'a, B>
where B: UsbBus {
    port: SerialPort<'a, B>,
    //timer: Timer,
    //last_hello: Instant,
    cmd_buf: CircularBuffer<64, u8>,
}

impl<'a, B: UsbBus> SerialIf<'a, B> {
    pub fn setup<'alloc: 'a>(usb_bus: &'alloc UsbBusAllocator<B>, timer: Timer) -> Self {
        SerialIf {
            port: SerialPort::new(&usb_bus),
            //timer,
            //last_hello: timer.get_counter(),
            cmd_buf: CircularBuffer::new(),
        }
    }

    pub fn poll(&mut self, usb_dev: &mut UsbDevice<B>) {
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
                    let wr_ptr = &buf[..count];
                    // TODO: Error if dropping command characters
                    self.cmd_buf.extend_from_slice(wr_ptr);
                    if let Some(line_end) = self.cmd_buf.iter().position(|&x| x == b'\r') {
                        self.cmd_buf.push_back(b'\n');
                        let contig = self.cmd_buf.make_contiguous();
                        match self.port.write(&contig[..=(line_end+1)]) {
                            Ok(_len) => {},
                            // On error, just drop unwritten data.
                            // One possible error is Err(WouldBlock), meaning the USB
                            // write buffer is full.
                            Err(_) => {},
                        }
                        self.cmd_buf.truncate_front(self.cmd_buf.len() - line_end - 2);
                    }

                    //let mut s: String::<64> = String::new();
                    //writeln!(s, "Buf Len: {}, end: {}\r", self.cmd_buf.len(), Into::<u64>::into(self.cmd_buf.back().cloned().unwrap_or(0)));
                    //self.port.write(&s.as_bytes());
                }
            }
        }
    }
}
