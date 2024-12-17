use defmt::{info, warn};
use embassy_boot::{AlignedBuffer, FirmwareUpdater, FirmwareUpdaterConfig};
use embassy_rp::{
    dma::AnyChannel,
    flash::{self, Flash},
    Peripheral, PeripheralRef,
};
use embassy_sync::{
    channel::{Channel, Receiver, Sender},
    mutex::{Mutex, MutexGuard},
};
use embedded_storage_async::nor_flash::NorFlash;
use heapless::Vec;
use picodox_proto::DATA_COUNT;

use crate::util::MutexType;

const FLASH_SIZE: usize = 8 * 1024 * 1024;
pub const FLASH_WRITE_BLOCK: usize = 4 * 1024;

pub struct FirmwareState {
    channel: Channel<MutexType, FirmwareCmd, 4>,
}

impl FirmwareState {
    pub fn new() -> Self {
        FirmwareState {
            channel: Channel::new(),
        }
    }

    pub fn get_intf<'d>(&'d self) -> FirmwareIntf<'d> {
        FirmwareIntf::new(self.channel.sender())
    }
}

pub struct FirmwareIntf<'d> {
    mutex: Mutex<MutexType, Sender<'d, MutexType, FirmwareCmd, 4>>,
}

impl<'d> FirmwareIntf<'d> {
    fn new(send: Sender<'d, MutexType, FirmwareCmd, 4>) -> Self {
        FirmwareIntf {
            mutex: Mutex::new(send),
        }
    }

    pub async fn lock<'a>(&'a self, initial_offset: u32) -> FirmwareSession<'a, 'd> {
        let guard = self.mutex.lock().await;

        FirmwareSession {
            guard,
            offset: initial_offset,
            data: Vec::new(),
        }
    }
}

pub struct FirmwareSession<'a, 'd> {
    guard: MutexGuard<'a, MutexType, Sender<'d, MutexType, FirmwareCmd, 4>>,
    offset: u32,
    data: Vec<u8, FLASH_WRITE_BLOCK>,
}

impl<'a, 'd> FirmwareSession<'a, 'd> {
    pub async fn begin(&mut self) {
        self.guard.send(FirmwareCmd::Begin).await;
    }

    pub async fn finish(&mut self) {
        if !self.data.is_empty() {
            self.write_block().await;
        }
        self.guard.send(FirmwareCmd::Finish).await;
    }

    pub async fn write(&mut self, data: &[u8; DATA_COUNT]) {
        if self.data.is_full() {
            self.write_block().await;
        }
        async_unwrap!(res self.data.extend_from_slice(data),
            "Block size not divisible by DATA_COUNT {}");
    }

    pub async fn set_offset(&mut self, offset: u32) {
        if !self.data.is_empty() {
            self.write_block().await;
        }
        self.offset = offset;
    }

    async fn write_block(&mut self) {
        let mut fixed_size = AlignedBuffer([0u8; FLASH_WRITE_BLOCK]);
        fixed_size.0[..self.data.len()].copy_from_slice(&self.data);
        self.guard
            .send(FirmwareCmd::Block(FirmwareBlock {
                data: fixed_size,
                offset: self.offset,
            }))
            .await;
        self.offset += FLASH_WRITE_BLOCK as u32;
        self.data.clear();
    }
}

enum FirmwareCmd {
    Begin,
    Finish,
    Block(FirmwareBlock),
}

struct FirmwareBlock {
    data: AlignedBuffer<FLASH_WRITE_BLOCK>,
    offset: u32,
}

pub struct FirmwareRecvr<'d, F: flash::Instance> {
    buffer: AlignedBuffer<8>,
    flash: PeripheralRef<'d, F>,
    dma: PeripheralRef<'d, AnyChannel>,
    cmd_recv: Receiver<'d, MutexType, FirmwareCmd, 4>,
}

impl<'d, F> FirmwareRecvr<'d, F>
where
    F: flash::Instance + Peripheral<P = F>,
{
    pub fn new<'a>(
        flash_p: F,
        dma_p: impl Peripheral<P = AnyChannel> + 'd,
        state: &'d FirmwareState,
    ) -> Self {
        let buffer = AlignedBuffer([0; 8]);

        let flash = flash_p.into_ref();
        let dma = dma_p.into_ref();

        let cmd_recv = state.channel.receiver();

        Self {
            buffer,
            flash,
            dma,
            cmd_recv,
        }
    }

    pub async fn run(self) -> ! {
        let flash = Flash::<_, _, FLASH_SIZE>::new(self.flash, self.dma);
        let flash_mutex = Mutex::new(flash);
        let config = FirmwareUpdaterConfig::from_linkerfile(&flash_mutex, &flash_mutex);
        let mut aligned = AlignedBuffer([0; 1]);
        let mut updater = FirmwareUpdater::new(config, &mut aligned.0);
        loop {
            loop {
                match self.cmd_recv.receive().await {
                    FirmwareCmd::Begin => break,
                    FirmwareCmd::Finish => warn!("Spurious FirmwareCmd::Finish received"),
                    FirmwareCmd::Block(_) => warn!("Spurious FirmwareCmd::Block(_) received"),
                }
            }

            let writer = async_unwrap!(res updater.prepare_update().await,
                "Error preparing for DFU update: {}");
            loop {
                match self.cmd_recv.receive().await {
                    FirmwareCmd::Begin => warn!("Second DFU started without finishing first"),
                    FirmwareCmd::Finish => break,
                    FirmwareCmd::Block(block) => {
                        info!("Writing block at offset {}", block.offset);
                        async_unwrap!(res writer.write(block.offset, &block.data.0[..]).await,
                            "Failed to write block to offset {}: {}", block.offset);
                    }
                }
            }

            async_unwrap!(res updater.mark_updated().await,
                "Failed to mark firmware as updated: {}");
        }
    }
}
