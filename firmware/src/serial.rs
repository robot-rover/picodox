
use circular_buffer::CircularBuffer;
use embassy_rp::rom_data;
use embassy_time::Timer;
use embassy_usb::{class::cdc_acm::{CdcAcmClass, State}, driver::Driver, Builder};
use picodox_proto::{errors::Ucid, AckType, Command, Response, WireSize};
// USB Communications Class Device support

use picodox_proto::proto_impl;

const SERIAL_DC_UCID: Ucid = Ucid(0x1);
const SERIAL_ERR_UCID: Ucid = Ucid(0x2);
const SERIAL_ECHO_UCID: Ucid = Ucid(0x3);
const SERIAL_DATA_UCID: Ucid = Ucid(0x4);

const MAX_PACKET_SIZE: usize = 64;

pub struct SerialIf<'d, D>
where D: Driver<'d> {
    class: CdcAcmClass<'d, D>,
    coms_buf: CircularBuffer<{ 2 * MAX_PACKET_SIZE }, u8>,
    pack_buf: [u8; MAX_PACKET_SIZE],
}

impl<'d, D: Driver<'d>> SerialIf<'d, D> {
    pub fn new(builder: &mut Builder<'d, D>, state: &'d mut State<'d>) -> Self {
        SerialIf {
            class: CdcAcmClass::new(builder, state, MAX_PACKET_SIZE as u16),
            coms_buf: CircularBuffer::new(),
            pack_buf: [0u8; MAX_PACKET_SIZE],
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let count = loop {
                match self.class.read_packet(&mut self.pack_buf).await {
                    Ok(count) => break count,
                    Err(_e) => {},
                }
            };

            // TODO: Error if dropping command characters
            self.coms_buf.extend_from_slice(&self.pack_buf[..count]);

            if let Some(line_end) = self.coms_buf.iter().position(|&x| x == 0u8) {
                let contig = self.coms_buf.make_contiguous();
                let message_buf = &mut contig[..=line_end];
                // TODO: Error handling
                let message = match proto_impl::wire_decode::<Command>(SERIAL_DC_UCID, message_buf) {
                    Ok(msg) => msg,
                    Err(err) => {
                        self.send_packet(Response::PacketErr(err), SERIAL_ERR_UCID).await;
                        self.coms_buf.truncate_front(self.coms_buf.len() - line_end - 1);
                        continue
                    },
                };
                self.coms_buf.truncate_front(self.coms_buf.len() - line_end - 1);

                match message {
                    Command::Reset => {
                        self.send_packet(Response::Ack(AckType::AckReset), SERIAL_ECHO_UCID).await;
                        Timer::after_secs(1).await;
                        rom_data::reset_to_usb_boot(0, 0);
                        loop {}
                    },
                    Command::FlashFw => {
                        self.send_packet(Response::Ack(AckType::AckFlash), SERIAL_ECHO_UCID).await;
                        Timer::after_secs(1).await;
                        rom_data::reset_to_usb_boot(0, 0);
                        loop {}
                    },
                    Command::EchoMsg { count } => {
                        self.send_packet(Response::EchoMsg { count }, SERIAL_ECHO_UCID).await;
                    },
                    Command::Data(data) => {
                        self.send_packet(Response::Data(data), SERIAL_DATA_UCID).await;
                    },
                }
            }
        }
    }

    async fn send_packet(&mut self, response: Response, ucid: Ucid) {
        match proto_impl::wire_encode::<_, { Response::WIRE_MAX_SIZE }>(ucid, response) {
            Ok(buf) => self.send_buf(&buf).await,
            Err(_err) => self.send_buf(&[0xBE, 0xEF, ucid.0, 0x00]).await,
        };
    }

    async fn send_buf(&mut self, buf: &[u8]) {
        let mut chunks_exact = buf.chunks_exact(MAX_PACKET_SIZE);

        for chunk in chunks_exact.by_ref() {
            self.class.write_packet(chunk).await;
        }

        self.class.write_packet(chunks_exact.remainder()).await;
    }
}

