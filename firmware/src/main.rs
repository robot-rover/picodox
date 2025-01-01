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

#[macro_use]
mod util;

mod dfu;
mod key_codes;
mod key_matrix;
mod keyboard;
mod logging;
mod neopixel;
mod serial;

use core::sync::atomic::Ordering;

use defmt::{error, info, println};
use embassy_futures::select::select;
use embassy_rp::dma::AnyChannel;
use embassy_rp::gpio::Pin;
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use embassy_time::Timer;
use key_matrix::{NUM_COLS, NUM_ROWS};
use keyboard::KeyboardIf;
use logging::{LoggerIf, LoggerRxSink};
use neopixel::{Color, Neopixel};
use panic_halt as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{FLASH, PIO0, USB};
use embassy_rp::pio::{self, Pio};
use embassy_rp::usb::{self, Driver};
use embassy_usb::class::{cdc_acm, hid};
use embassy_usb::{Config, Handler, UsbDevice};
use portable_atomic::AtomicBool;
use serial::SerialIf;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

static INITIATE_SHUTDOWN: Watch<CriticalSectionRawMutex, (), 1> = Watch::new();
static USB_SHUTDOWN: Signal<CriticalSectionRawMutex, ()> = Signal::new();

enum Hand {
    Left,
    Right,
}

#[cfg(not(feature = "right"))]
const THIS_HAND: Hand = Hand::Left;
#[cfg(feature = "right")]
const THIS_HAND: Hand = Hand::Right;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // TODO: Cleanup below
    let p = embassy_rp::init(Default::default());
    // Disable the watchdog from the bootloader
    embassy_rp::pac::WATCHDOG.ctrl().write(|w| w.set_enable(false));
    let watchdog = Watchdog::new(p.WATCHDOG);


    // Create the driver, from the HAL.
    let driver = usb::Driver::new(p.USB, Irqs);

    // Create embassy-usb Config
    let config = {
        const USB_VID: u16 = 0x08B9;
        const USB_PID: u16 = 0xBEEF;

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

    //let dfu_state = {
    //    static DFU_STATE: StaticCell<FirmwareState> = StaticCell::new();
    //    DFU_STATE.init(FirmwareState::new())
    //};

    // Create classes on the builder.
    let serial = {
        static STATE: StaticCell<cdc_acm::State> = StaticCell::new();
        let state = STATE.init(Default::default());
        SerialIf::new(
            &mut builder,
            state,
            watchdog,
            //dfu_state.get_intf(),
        )
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

    //let dfu = FirmwareRecvr::new(p.FLASH, AnyChannel::from(p.DMA_CH1), dfu_state);

    // p.PIN_19 is rotary encoder momentary switch

    let keyboard = {
        static STATE: StaticCell<hid::State> = StaticCell::new();
        let state = STATE.init(Default::default());
        // Row Pins (from kb2040 pin numbers)
        // [1, 2, 7, 8, 9]
        let row_pins = [
            p.PIN_0.degrade(),
            p.PIN_1.degrade(),
            p.PIN_4.degrade(),
            p.PIN_5.degrade(),
            p.PIN_6.degrade(),
        ];
        // Col Pins (from kb2040 pin numbers)
        // [17, 18, 19, 20, 12, 11, 10]
        let col_pins = [
            p.PIN_26.degrade(),
            p.PIN_27.degrade(),
            p.PIN_28.degrade(),
            p.PIN_29.degrade(),
            p.PIN_9.degrade(),
            p.PIN_8.degrade(),
            p.PIN_7.degrade(),
        ];
        KeyboardIf::new(&mut builder, state, col_pins, row_pins)
    };

    static DEVICE_HANDLER: StaticCell<MyDeviceHandler> = StaticCell::new();
    builder.handler(DEVICE_HANDLER.init(MyDeviceHandler::new()));

    // Build the usb device
    let usb = builder.build();

    spawner.must_spawn(serial_task(serial));
    spawner.must_spawn(logger_task(logger));
    spawner.must_spawn(logger_rx_task(logger_rx));
    spawner.must_spawn(usb_task(usb));
    spawner.must_spawn(neopixel_task(neopixel));
    spawner.must_spawn(hello_task(&LED_SIGNAL));
    spawner.must_spawn(keyboard_task(keyboard));
    //spawner.must_spawn(dfu_task(dfu));
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
async fn keyboard_task(keyboard: KeyboardIf<'static, Driver<'static, USB>, NUM_ROWS, NUM_COLS>) {
    keyboard.run().await;
}

#[embassy_executor::task]
async fn hello_task(led_signal: &'static Signal<CriticalSectionRawMutex, Color>) -> ! {
    let mut i = 0usize;
    let mut b = false;
    loop {
        if i % 10 == 0 {
            println!("Hello World #{} :-)", i / 10);
            //println!("Handed loc: {}", unsafe { __keyboard_meta_start } );

            extern "C" {
                static __keyboard_meta_start: u32;
                static __keyboard_meta_end: u32;
            }

            unsafe {
                let start = &__keyboard_meta_start as *const u32 as u32;
                let end = &__keyboard_meta_end as *const u32 as u32;
                const XIP_BASE: u32 = 0x10000000;
                println!("Handed: 0x{:x} = {}", start, *((start | XIP_BASE) as *const u8));
            }
            defmt::flush();
            b = !b;
        }

        led_signal.signal(if b {
            Color::wheel(i as u8)
        } else {
            Color::new(0, 0, 0)
        });
        Timer::after_millis(100).await;
        i = i.wrapping_add(1);
    }

}

//#[embassy_executor::task]
//async fn dfu_task(dfu: FirmwareRecvr<'static, FLASH>) -> ! {
//    dfu.run().await
//}

//TODO: Cleanup Below
struct MyDeviceHandler {
    configured: AtomicBool,
}

impl MyDeviceHandler {
    fn new() -> Self {
        MyDeviceHandler {
            configured: AtomicBool::new(false),
        }
    }
}

impl Handler for MyDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        self.configured.store(false, Ordering::Relaxed);
        if enabled {
            info!("Device enabled");
        } else {
            info!("Device disabled");
        }
    }

    fn reset(&mut self) {
        self.configured.store(false, Ordering::Relaxed);
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        self.configured.store(false, Ordering::Relaxed);
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        self.configured.store(configured, Ordering::Relaxed);
        if configured {
            info!(
                "Device configured, it may now draw up to the configured current limit from Vbus."
            )
        } else {
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }
}
