
use circular_buffer::CircularBuffer;
use embassy_rp::{rom_data, watchdog::{Watchdog}};
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
    watchdog: Watchdog,
}

impl<'d, D: Driver<'d>> SerialIf<'d, D> {
    pub fn new(builder: &mut Builder<'d, D>, state: &'d mut State<'d>, watchdog:  Watchdog) -> Self {
        SerialIf {
            class: CdcAcmClass::new(builder, state, MAX_PACKET_SIZE as u16),
            coms_buf: CircularBuffer::new(),
            pack_buf: [0u8; MAX_PACKET_SIZE],
            watchdog,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let count = async_unwrap!(res self.class.read_packet(&mut self.pack_buf).await, "Usb read_packet error: {}");

            // TODO: Error if dropping command characters
            if self.coms_buf.capacity() - self.coms_buf.len() < count {

            }
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
                        crate::shutdown().await;
                        self.watchdog.trigger_reset();
                        loop {}
                    },
                    Command::UsbDfu => {
                        self.send_packet(Response::Ack(AckType::AckUsbDfu), SERIAL_ECHO_UCID).await;
                        crate::shutdown().await;
                        rom_data::reset_to_usb_boot(0, 0);
                        loop {}
                    },
                    Command::EchoMsg { count } => {
                        self.send_packet(Response::EchoMsg { count }, SERIAL_ECHO_UCID).await;
                    },
                    Command::Data(data) => {
                        self.send_packet(Response::Data(data), SERIAL_DATA_UCID).await;
                    },
                    Command::FlashFw { count } => todo!(),
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

        // These two errors will only occur if an endpoint is disabled or
        // if the send/recv buffer is too small, both should be considered
        // unrecoverable as they require a recompile to fix
        for chunk in chunks_exact.by_ref() {
            async_unwrap!(res self.class.write_packet(chunk).await, "Error sending buffer: {}");
        }

        async_unwrap!(res self.class.write_packet(chunks_exact.remainder()).await, "Error sending buffer: {}");
    }
}

