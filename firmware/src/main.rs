//! # Pico Blinky Example
//!
//! Blinks the LED on a Pico board.
//!
//! This will blink an LED attached to GP25, which is the pin the Pico uses for
//! the on-board LED.
//!
//! See the `Cargo.toml` file for Copyright and license details.

#![no_std]
#![no_main]

mod serial;
mod proto_impl;

use embassy_rp::watchdog::Watchdog;
use panic_halt as _;

use embassy_executor::Spawner;
use embassy_usb::driver::EndpointError;
use embassy_usb::{Config, UsbDevice};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_rp::usb;
use embassy_rp::peripherals::USB;
use embassy_rp::bind_interrupts;
use serial::SerialIf;
use static_cell::StaticCell;

//static WATCHDOG : embassy_rp::watchdog::Mutex<ThreadModeWatchdog = StaticCell::new(embassy_rp::watchdog::Watchdog::new());

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let watch = Watchdog::new(p.WATCHDOG);

    // Create the driver, from the HAL.
    let driver = usb::Driver::new(p.USB, Irqs);

    // Create embassy-usb Config
    let config = {
        const USB_VID : u16 = 0x08B9;
        const USB_PID : u16 = 0xBEEF;

        let mut config = Config::new(USB_VID, USB_PID);
        config.device_class = 0; // from: https://www.usb.org/defined-class-codes
        config.manufacturer = Some("rr Industries");
        config.product = Some("Picodox Keyboard");
        config.serial_number = Some("0000-0001");
        config.max_power = 100; // mA
        config.max_packet_size_0 = 64;

        config
    };

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

        let builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [], // no msos descriptors
            CONTROL_BUF.init([0; 64]),
        );
        builder
    };

    // Create classes on the builder.
    let mut serial = {
        static STATE: StaticCell<State> = StaticCell::new();
        let state = STATE.init(State::new());
        SerialIf::setup(&mut builder, state, watch)
    };

    // Build the builder.
    let usb = builder.build();

    // Run the USB device.
    let _unit = if let Ok(unit) = spawner.spawn(usb_task(usb)) {
        unit
    } else {
        panic!();
    };

    // Do stuff with the class!

    serial.run().await;
    loop {}
}

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, usb::Driver<'static, USB>>) -> ! {
    usb.run().await
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

