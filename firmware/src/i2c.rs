use defmt::{info, println, warn};
use embassy_futures::select::{select, Either};
use embassy_rp::{i2c::{self, Async, Config, I2c, Instance, InterruptHandler, SclPin, SdaPin}, i2c_slave::{self, Command}, interrupt::typelevel::Binding, Peripheral};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use heapless::Vec;
use picodox_proto::{proto_impl, KeyUpdate, WireSize};

use crate::util::MutexType;


pub struct I2cMaster<'d, T: Instance> {
    bus: I2c<'d, T, Async>,
    signal: &'d Signal<MutexType, KeyUpdate>,
}

impl<'d, T: Instance> I2cMaster<'d, T> {
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = impl SclPin<T>> + 'd,
        sda: impl Peripheral<P = impl SdaPin<T>> + 'd,
        irq: impl Binding<T::Interrupt, InterruptHandler<T>>,
        signal: &'d Signal<MutexType, KeyUpdate>,
    ) -> Self {
        let config = Config::default();
        let bus = I2c::new_async(peri, scl, sda, irq, config);

        I2cMaster { bus, signal }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let ku = self.signal.wait().await;
            println!("I2C Master rx'd key update");
            let buffer: Vec<u8, { KeyUpdate::CS_MAX_SIZE }> = match proto_impl::cs_encode(&ku) {
                Ok(b) => b,
                Err(e) => {
                    defmt::error!("I2C Encode Error: {:?}", e);
                    continue
                }
            };
            println!("I2C Master encoded update, sending...");
            if let Err(e) = self.bus.write_async(0x55u16, buffer).await {
                defmt::warn!("I2C Error: {:?}", e);
                continue
            }

        }
    }
}

pub struct I2cSlave<'d, T: Instance> {
    bus: i2c_slave::I2cSlave<'d, T>,
    signal: &'d Signal<MutexType, KeyUpdate>,
}

impl <'d, T: Instance> I2cSlave<'d, T> {
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = impl SclPin<T>> + 'd,
        sda: impl Peripheral<P = impl SdaPin<T>> + 'd,
        irq: impl Binding<T::Interrupt, InterruptHandler<T>>,
        signal: &'d Signal<MutexType, KeyUpdate>,
    ) -> Self {
        let mut config = i2c_slave::Config::default();
        config.addr = 0x55u16;
        let bus = i2c_slave::I2cSlave::new(peri, scl, sda, irq, config);

        I2cSlave { bus, signal }
    }

    pub async fn run(&mut self) -> ! {
        let mut buffer = [0u8; KeyUpdate::CS_MAX_SIZE];

        loop {
            match self.bus.listen(&mut buffer).await {
                Ok(event) => match event {
                    Command::GeneralCall(_) | Command::WriteRead(_) | Command::Read  => { warn!("Rv'd unexpected I2C") },
                    Command::Write(len) => {
                        info!("Rv'd I2C Write");
                        let key_update: KeyUpdate = match proto_impl::cs_decode(&mut buffer[..len]) {
                            Ok(ku) => ku,
                            Err(e) => {
                                defmt::error!("I2C Decode Error: {:?}", e);
                                continue
                            }
                        };

                        self.signal.signal(key_update);
                    },
                },
                Err(e) => {
                    defmt::error!("I2C Slave Error: {:?}", e);
                    continue
                },
            }
        }
    }
}
