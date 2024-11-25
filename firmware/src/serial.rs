use core::fmt::Write as _;

use circular_buffer::CircularBuffer;
use heapless::String;
use picodox_proto::{errors::Ucid, Command, Response, WireSize};
use rp_pico::hal::Timer;
use usb_device::{bus::{UsbBus, UsbBusAllocator}, device::UsbDevice};
// USB Communications Class Device support
use usbd_serial::SerialPort;

use crate::proto_impl;

const SERIAL_DC_UCID: Ucid = Ucid(0x1);
const SERIAL_ERR_UCID: Ucid = Ucid(0x2);
const SERIAL_ECHO_UCID: Ucid = Ucid(0x3);
const SERIAL_DATA_UCID: Ucid = Ucid(0x4);

pub struct SerialIf<'a, B>
where B: UsbBus {
    port: SerialPort<'a, B>,
    //timer: Timer,
    //last_hello: Instant,
    cmd_buf: CircularBuffer<64, u8>,
    current_command: Option<Command>,
}

impl<'a, B: UsbBus> SerialIf<'a, B> {
    pub fn setup<'alloc: 'a>(usb_bus: &'alloc UsbBusAllocator<B>, timer: Timer) -> Self {
        SerialIf {
            port: SerialPort::new(&usb_bus),
            //timer,
            //last_hello: timer.get_counter(),
            cmd_buf: CircularBuffer::new(),
            current_command: None,
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
                    if let Some(line_end) = self.cmd_buf.iter().position(|&x| x == 0u8) {
                        let contig = self.cmd_buf.make_contiguous();
                        let message_buf = &mut contig[..=line_end];
                        // TODO: Error handling
                        let message = match proto_impl::wire_decode::<Command>(SERIAL_DC_UCID, message_buf) {
                            Ok(msg) => msg,
                            Err(err) => {
                                self.send_packet(Response::PacketErr(err), SERIAL_ERR_UCID);
                                self.cmd_buf.truncate_front(self.cmd_buf.len() - line_end - 1);
                                return;
                            },
                        };
                        self.cmd_buf.truncate_front(self.cmd_buf.len() - line_end - 1);

                        match message {
                            Command::Reset => rp_pico::hal::reset(),
                            Command::FlashFw => rp_pico::hal::rom_data::reset_to_usb_boot(0, 0),
                            Command::EchoMsg { count } => {
                                self.send_packet(Response::EchoMsg { count }, SERIAL_ECHO_UCID);
                            },
                            Command::Data(data) => {
                                self.send_packet(Response::Data(data), SERIAL_DATA_UCID);
                            },
                        }
                    }

                    //let mut s: String::<64> = String::new();
                    //writeln!(s, "Buf Len: {}, end: {}\r", self.cmd_buf.len(), Into::<u64>::into(self.cmd_buf.back().cloned().unwrap_or(0)));
                    //self.port.write(&s.as_bytes());
                }
            }
        }
    }

    fn send_packet(&mut self, response: Response, ucid: Ucid) {
        match proto_impl::wire_encode::<_, { Response::WIRE_MAX_SIZE }>(ucid, response) {
            Ok(buf) => self.port.write(&buf),
            Err(err) => self.port.write(&[0xBE, 0xEF, ucid.0, 0x00]),
        };
    }
}
