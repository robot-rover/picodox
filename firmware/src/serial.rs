
use circular_buffer::CircularBuffer;
use defmt::{error, info};
use embassy_rp::{pac::{TIMER, WATCHDOG}, peripherals::WATCHDOG, rom_data, time_driver::visit_timer_queue, watchdog::Watchdog};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    driver::Driver,
    Builder,
};
use heapless::Vec;
use picodox_proto::{Command, NackType, Response, TaskData, TimerDebug, WireSize, DATA_COUNT};
// USB Communications Class Device support

use picodox_proto::proto_impl;

//use crate::dfu::{FirmwareIntf, FirmwareSession};

const MAX_PACKET_SIZE: usize = 64;

pub struct SerialIf<'d, D>
where
    D: Driver<'d>,
{
    packet: Packetizer<'d, D>,
}

pub struct Packetizer<'d, D>
where
    D: Driver<'d>,
{
    class: CdcAcmClass<'d, D>,
    coms_buf: CircularBuffer<{ 2 * MAX_PACKET_SIZE }, u8>,
    pack_buf: [u8; MAX_PACKET_SIZE],
}

fn now() -> u64 {
    loop {
        let hi = TIMER.timerawh().read();
        let lo = TIMER.timerawl().read();
        let hi2 = TIMER.timerawh().read();
        if hi == hi2 {
            return (hi as u64) << 32 | (lo as u64);
        }
    };
}

impl<'d, D: Driver<'d>> Packetizer<'d, D> {
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

    async fn recv_data<F>(&mut self, count: u32, callback: &mut F)
    where
        F: DataRecvr<'d, D>,
    {
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

pub trait DataRecvr<'d, D: Driver<'d>> {
    async fn callback(&mut self, p: &mut Packetizer<'d, D>, data: &[u8; DATA_COUNT]);
}

struct EchoRecvr;

impl<'d, D: Driver<'d>> DataRecvr<'d, D> for EchoRecvr {
    async fn callback(&mut self, p: &mut Packetizer<'d, D>, data: &[u8; DATA_COUNT]) {
        p.send_packet(&Response::Data(*data)).await;
    }
}

impl<'d, D: Driver<'d>> SerialIf<'d, D> {
    pub fn new(
        builder: &mut Builder<'d, D>,
        state: &'d mut State<'d>,
    ) -> Self {
        let packet = Packetizer {
            class: CdcAcmClass::new(builder, state, MAX_PACKET_SIZE as u16),
            coms_buf: CircularBuffer::new(),
            pack_buf: [0u8; MAX_PACKET_SIZE],
        };

        SerialIf { packet }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let res = self.packet.recv_cmd().await;
            let message = match res {
                Ok(cmd) => cmd,
                Err(reason) => {
                    // In case of an error here, just respond with an error
                    self.packet.send_packet(&Response::Nack(reason)).await;
                    continue;
                }
            };
            match message {
                Command::Reset => {
                    crate::shutdown().await;
                    // Safety: this is safe as code will never return from this function
                    let mut watchdog = Watchdog::new(unsafe { WATCHDOG::steal() });
                    watchdog.trigger_reset();
                    loop {}
                }
                Command::UsbDfu => {
                    crate::shutdown().await;
                    rom_data::reset_to_usb_boot(0, 0);
                    loop {}
                }
                Command::EchoMsg { count } => {
                    self.packet.send_packet(&Response::EchoMsg { count }).await;
                    self.packet.recv_data(count as u32, &mut EchoRecvr).await;
                }
                Command::Data(_data) => {
                    self.packet
                        .send_packet(&Response::Nack(NackType::Unexpected))
                        .await;
                }
                Command::TimerDebug => {
                    let current_time = now();
                    let mut tasks: Vec<TaskData, 32> = Vec::new();
                    critical_section::with(|cs| {
                        visit_timer_queue(cs, |item| {
                            let td = TaskData {
                                state: item.header().state.state.borrow(cs).get(),
                                loc: item.header() as *const _ as u32,
                                expires_at: item.timer_queue_item().expires_at.get(),
                            };
                            let _ = tasks.push(td);
                        });
                    });

                    let td = TimerDebug {
                        current_time,
                        fire_time: TIMER.alarm(0).read(),
                        armed: TIMER.armed().read().0 & 0x1u32 != 0,
                        enabled: TIMER.inte().read().0 & 0x1u32 != 0,
                        tasks,
                    };
                    self.packet
                        .send_packet(&Response::TimerDebug(td))
                        .await;
                }
            }
        }
    }
}

