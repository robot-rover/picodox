use circular_buffer::CircularBuffer;
use defmt::error;
use embassy_rp::{rom_data, watchdog::Watchdog};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    driver::Driver,
    Builder,
};
use picodox_proto::{AckType, Command, NackType, Response, WireSize, DATA_COUNT};
// USB Communications Class Device support

use picodox_proto::proto_impl;

const MAX_PACKET_SIZE: usize = 64;

pub struct SerialIf<'d, D>
where
    D: Driver<'d>,
{
    class: CdcAcmClass<'d, D>,
    coms_buf: CircularBuffer<{ 2 * MAX_PACKET_SIZE }, u8>,
    pack_buf: [u8; MAX_PACKET_SIZE],
    watchdog: Watchdog,
}

trait DataRecvr<'d, D: Driver<'d>> {
    async fn callback(&mut self, s: &mut SerialIf<'d, D>, data: &[u8; DATA_COUNT]);
}

struct EchoRecvr;

impl<'d, D: Driver<'d>> DataRecvr<'d, D> for EchoRecvr {
    async fn callback(&mut self, s: &mut SerialIf<'d, D>, data: &[u8; DATA_COUNT]) {
        s.send_packet(&Response::Data(*data)).await
    }
}

impl<'d, D: Driver<'d>> SerialIf<'d, D> {
    pub fn new(builder: &mut Builder<'d, D>, state: &'d mut State<'d>, watchdog: Watchdog) -> Self {
        SerialIf {
            class: CdcAcmClass::new(builder, state, MAX_PACKET_SIZE as u16),
            coms_buf: CircularBuffer::new(),
            pack_buf: [0u8; MAX_PACKET_SIZE],
            watchdog,
        }
    }

    async fn recv_cmd(&mut self) -> Result<Command, NackType> {
        let mut lost_bytes = false;
        let line_end = loop {
            // Check if we have enough bytes already
            if let Some(line_end) = self.coms_buf.iter().position(|&x| x == 0u8) {
                if lost_bytes {
                    // Remove truncated packet from the buffer
                    self.coms_buf
                        .truncate_front(self.coms_buf.len() - line_end - 1);
                    return Err(NackType::BufferOverflow);
                } else {
                    break line_end;
                }
            }

            // Otherwise, wait for another packet
            let count = async_unwrap!(res self.class.read_packet(&mut self.pack_buf).await,
                "Usb read_packet error: {}");

            // Log an error if we overflow our buffer
            let open_cap = self.coms_buf.capacity() - self.coms_buf.len();
            if open_cap < count {
                error!("Dropping command packet bytes (coms_buf has {} open bytes, pack_buf has {} bytes)",
                    open_cap, count);
                lost_bytes = true;
            }
            self.coms_buf.extend_from_slice(&self.pack_buf[..count]);
        };

        // Now we have at least a whole packet in the buffer
        let contig = self.coms_buf.make_contiguous();
        let message_buf = &mut contig[..=line_end];

        let decoded = proto_impl::wire_decode::<Command>(message_buf);
        // Remove the decoded bytes from the circular buffer
        self.coms_buf
            .truncate_front(self.coms_buf.len() - line_end - 1);

        decoded.map_err(|err| NackType::PacketErr(err))
    }

    async fn recv_data<F: DataRecvr<'d, D>>(&mut self, count: u16, mut callback: F) {
        for _bytes_recieved in (0..count).step_by(DATA_COUNT) {
            let res = self.recv_cmd().await.and_then(|cmd| match cmd {
                Command::Data(data) => Ok(data),
                _ => Err(NackType::Unexpected),
            });
            match res {
                Ok(data) => callback.callback(self, &data).await,
                Err(reason) => {
                    self.send_packet(&Response::Nack(reason)).await;
                    continue;
                }
            }
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let res = self.recv_cmd().await;
            let message = match res {
                Ok(cmd) => cmd,
                Err(reason) => {
                    // In case of an error here, just respond with an error
                    self.send_packet(&Response::Nack(reason)).await;
                    continue;
                }
            };
            match message {
                Command::Reset => {
                    self.send_packet(&Response::Ack(AckType::AckReset)).await;
                    crate::shutdown().await;
                    self.watchdog.trigger_reset();
                    loop {}
                }
                Command::UsbDfu => {
                    self.send_packet(&Response::Ack(AckType::AckUsbDfu)).await;
                    crate::shutdown().await;
                    rom_data::reset_to_usb_boot(0, 0);
                    loop {}
                }
                Command::EchoMsg { count } => {
                    self.send_packet(&Response::EchoMsg { count }).await;
                    self.recv_data(count, EchoRecvr).await;
                }
                Command::Data(_data) => {
                    self.send_packet(&Response::Nack(NackType::Unexpected))
                        .await;
                }
                Command::FlashFw { count } => {}
            }
        }
    }

    async fn send_packet(&mut self, response: &Response) {
        match proto_impl::wire_encode::<_, { Response::WIRE_MAX_SIZE }>(response) {
            Ok(buf) => self.send_buf(&buf).await,
            Err(_err) => self.send_buf(&[0xBE, 0xEF, 0x00]).await,
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
