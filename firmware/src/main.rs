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
mod logging;
mod neopixel;
mod keyboard;

use embassy_futures::select::select;
use embassy_rp::dma::AnyChannel;
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use embassy_time::Timer;
use keyboard::KeyboardIf;
use logging::{LoggerIf, LoggerRxSink};
use neopixel::{Color, Neopixel};
use panic_halt as _;

use embassy_executor::Spawner;
use embassy_rp::pio::{self, Pio};
use embassy_usb::{Config, UsbDevice};
use embassy_usb::class::{cdc_acm, hid};
use embassy_rp::usb::{self, Driver};
use embassy_rp::peripherals::{PIO0, USB};
use embassy_rp::bind_interrupts;
use serial::SerialIf;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

static INITIATE_SHUTDOWN: Watch<CriticalSectionRawMutex, (), 1> = Watch::new();
static USB_SHUTDOWN: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

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
    let serial = {
        static STATE: StaticCell<cdc_acm::State> = StaticCell::new();
        let state = STATE.init(Default::default());
        SerialIf::new(&mut builder, state, Watchdog::new(p.WATCHDOG))
    };

    let (logger, logger_rx) = {
        static STATE: StaticCell<cdc_acm::State> = StaticCell::new();
        let state = STATE.init(Default::default());
        logging::new(&mut builder, state)
    };

    static LED_SIGNAL: Signal<CriticalSectionRawMutex, Color> = Signal::new();
    let neopixel = {
        let pio0 = Pio::new(p.PIO0, Irqs);
        Neopixel::new(pio0, p.PIN_17, AnyChannel::from(p.DMA_CH0), &LED_SIGNAL)
    };

    let keyboard = {
        static STATE: StaticCell<hid::State> = StaticCell::new();
        let state = STATE.init(Default::default());
        //KeyboardIf::new(&mut builder, state, p.PIN_19)
    };

    // Build the usb device
    let usb = builder.build();

    spawner.must_spawn(serial_task(serial));
    spawner.must_spawn(logger_task(logger));
    spawner.must_spawn(logger_rx_task(logger_rx));
    spawner.must_spawn(usb_task(usb));
    spawner.must_spawn(neopixel_task(neopixel));
    spawner.must_spawn(hello_task(&LED_SIGNAL));
    //spawner.must_spawn(keyboard_task(keyboard));
}

async fn shutdown() {
    let shutdown_sender = INITIATE_SHUTDOWN.sender();
    shutdown_sender.send(());
    USB_SHUTDOWN.wait().await;
}

#[embassy_executor::task]
async fn serial_task(mut serial: SerialIf<'static, Driver<'static, USB>>) {
    serial.run().await;

}

#[embassy_executor::task]
async fn logger_task(mut logger: LoggerIf<'static, Driver<'static, USB>>) -> ! {
    logger.run().await
}

#[embassy_executor::task]
async fn logger_rx_task(mut logger_rx: LoggerRxSink<'static, Driver<'static, USB>>) -> ! {
    logger_rx.run().await
}

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) {
    let mut shutdown_receiver = INITIATE_SHUTDOWN.receiver().unwrap();
    let _unit = select(usb.run(), shutdown_receiver.get()).await;
    usb.disable().await;
    USB_SHUTDOWN.signal(());
}

#[embassy_executor::task]
async fn neopixel_task(mut neopixel: Neopixel<'static, PIO0>) -> ! {
    neopixel.run().await
}

#[embassy_executor::task]
async fn keyboard_task(keyboard: KeyboardIf<'static, Driver<'static, USB>>) {
    keyboard.run().await;
}

#[embassy_executor::task]
async fn hello_task(led_signal: &'static Signal<CriticalSectionRawMutex, Color>) -> ! {
    let mut i = 0;
    loop {
        if i % 10 == 0 {
            defmt::println!("Hello World #{} :-)", i / 10);
            defmt::flush();
        }
        led_signal.signal(Color::wheel(i as u8));
        Timer::after_millis(100).await;
        i += 1;
    }
}
