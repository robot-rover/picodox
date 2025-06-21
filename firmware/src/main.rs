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

mod i2c;
mod key_codes;
mod key_hid;
mod key_map;
mod key_matrix;
mod logging;
mod neopixel;
mod panic_handler;
mod serial;

use core::sync::atomic::Ordering;

use defmt::{info, println};
use embassy_futures::select::select;
use embassy_rp::dma::AnyChannel;
use embassy_rp::gpio::{Input, Level, Pin, Pull};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use embassy_time::Timer;
use i2c::{I2cMaster, I2cSlave};
use key_hid::KeyboardIf;
use key_map::BasicKeymap;
use key_matrix::KeyMatrix;
use logging::{LoggerIf, LoggerRxSink};
use neopixel::{Color, Neopixel};

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{I2C1, PIO0, USB};
use embassy_rp::pio::{self, Pio};
use embassy_rp::usb::{self, Driver};
use embassy_usb::class::{cdc_acm, hid};
use embassy_usb::{Config, Handler, UsbDevice};
use picodox_proto::{KeyUpdate, NUM_COLS, NUM_ROWS};
use portable_atomic::AtomicBool;
use serial::SerialIf;
use static_cell::StaticCell;
use util::MutexType;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    I2C1_IRQ => embassy_rp::i2c::InterruptHandler<I2C1>;
});

static INITIATE_SHUTDOWN: Watch<CriticalSectionRawMutex, (), 1> = Watch::new();
static USB_SHUTDOWN: Signal<CriticalSectionRawMutex, ()> = Signal::new();

const UPDATE_RATE_MS: u32 = 20;

#[allow(dead_code)]
#[derive(PartialEq, Eq)]
enum Hand {
    Left,
    Right,
}

enum I2cDir<P: embassy_rp::i2c::Instance + 'static> {
    Master(I2cMaster<'static, P>),
    Slave(I2cSlave<'static, P>),
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // TODO: Cleanup below
    let p = embassy_rp::init(Default::default());
    //// Disable the watchdog from the bootloader
    embassy_rp::pac::WATCHDOG
        .ctrl()
        .write(|w| w.set_enable(false));
    //let reason = embassy_rp::pac::WATCHDOG.reason().read();
    //if reason.timer() {
    //    // Watchdog triggered last reset, go to dfu mode
    //    rom_data::reset_to_usb_boot(0, 0);
    //    loop {}
    //}
    //let mut watchdog = Watchdog::new(p.WATCHDOG);
    //watchdog.start(Duration::from_secs(1));

    // Create the driver, from the HAL.
    let driver = usb::Driver::new(p.USB, Irqs);

    let hand_pin = Input::new(p.PIN_10, Pull::None);
    let this_hand = match hand_pin.get_level() {
        Level::Low => Hand::Left,
        Level::High => Hand::Right,
    };

    // Create embassy-usb Config
    let config = {
        const USB_VID: u16 = 0x08B9;
        const USB_PID: u16 = 0xBEEF;

        let mut config = Config::new(USB_VID, USB_PID);
        config.device_class = 0; // from: https://www.usb.org/defined-class-codes
        config.manufacturer = Some("rr Industries");
        match this_hand {
            Hand::Left => config.product = Some("Picodox Keyboard (Left)"),
            Hand::Right => config.product = Some("Picodox Keyboard (Right)"),
        }
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
        SerialIf::new(&mut builder, state)
    };

    let (logger, logger_rx) = {
        static STATE: StaticCell<cdc_acm::State> = StaticCell::new();
        let state = STATE.init(Default::default());
        logging::new(&mut builder, state)
    };

    static LED_SIGNAL: StaticCell<Signal<MutexType, Color>> = StaticCell::new();
    let led_signal = &*LED_SIGNAL.init(Signal::new());
    let neopixel = {
        let pio0 = Pio::new(p.PIO0, Irqs);
        Neopixel::new(
            pio0,
            p.PIN_17,
            p.PIN_25,
            AnyChannel::from(p.DMA_CH0),
            led_signal,
        )
    };
    led_signal.signal(Color::new(0, 0, 0));

    // p.PIN_19 is rotary encoder momentary switch

    static LEFT_SIGNAL: StaticCell<Signal<MutexType, KeyUpdate>> = StaticCell::new();
    let left_signal = &*LEFT_SIGNAL.init(Signal::new());
    static RIGHT_SIGNAL: StaticCell<Signal<MutexType, KeyUpdate>> = StaticCell::new();
    let right_signal = &*RIGHT_SIGNAL.init(Signal::new());

    let key_mat = {
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
        let my_signal = match this_hand {
            Hand::Left => left_signal,
            Hand::Right => right_signal,
        };
        KeyMatrix::new(col_pins, row_pins, my_signal, UPDATE_RATE_MS)
    };

    let key_hid = if this_hand == Hand::Left {
        static STATE: StaticCell<hid::State> = StaticCell::new();
        let state = STATE.init(Default::default());

        Some(KeyboardIf::new(
            &mut builder,
            state,
            left_signal,
            right_signal,
            UPDATE_RATE_MS,
            BasicKeymap::default(),
        ))
    } else {
        None
    };

    static DEVICE_HANDLER: StaticCell<MyDeviceHandler> = StaticCell::new();
    builder.handler(DEVICE_HANDLER.init(MyDeviceHandler::new()));

    // Build the usb device
    let usb = builder.build();

    let i2c = {
        let sda = p.PIN_2;
        let scl = p.PIN_3;

        match this_hand {
            Hand::Left => {
                let i2c = I2cSlave::new(p.I2C1, scl, sda, Irqs, right_signal);
                I2cDir::Slave(i2c)
            }
            Hand::Right => {
                let i2c = I2cMaster::new(p.I2C1, scl, sda, Irqs, right_signal);
                I2cDir::Master(i2c)
            }
        }
    };

    spawner.must_spawn(serial_task(serial));
    spawner.must_spawn(logger_task(logger));
    spawner.must_spawn(logger_rx_task(logger_rx));
    spawner.must_spawn(usb_task(usb));
    spawner.must_spawn(neopixel_task(neopixel));
    //spawner.must_spawn(hello_task(&led_signal));
    spawner.must_spawn(key_mat_task(key_mat));
    //spawner.must_spawn(busy_task());

    if let Some(key_hid) = key_hid {
        spawner.must_spawn(key_hid_task(key_hid));
    };

    match i2c {
        I2cDir::Master(m) => spawner.must_spawn(i2c_master_task(m)),
        I2cDir::Slave(s) => spawner.must_spawn(i2c_slave_task(s)),
    };
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
async fn key_hid_task(keyboard: KeyboardIf<'static, Driver<'static, USB>, BasicKeymap>) {
    keyboard.run().await;
}

#[embassy_executor::task]
async fn key_mat_task(keyboard: KeyMatrix<'static, { NUM_ROWS }, { NUM_COLS }>) {
    keyboard.run().await;
}

#[embassy_executor::task]
async fn hello_task(led_signal: &'static Signal<MutexType, Color>) -> ! {
    let mut i = 0usize;
    let mut b = false;
    loop {
        if i % 10 == 0 {
            println!("Hello World #{} :-)", i / 10);
            //println!("Handed loc: {}", unsafe { __keyboard_meta_start } );
            b = !b;
        }

        led_signal.signal(if b {
            Color::wheel(i as u8)
        } else {
            Color::new(0, 0, 0)
        });
        i = i.wrapping_add(1);
        Timer::after_millis(100).await;
    }
}

#[embassy_executor::task]
async fn i2c_master_task(mut i2c: I2cMaster<'static, I2C1>) -> ! {
    i2c.run().await
}

#[embassy_executor::task]
async fn i2c_slave_task(mut i2c: I2cSlave<'static, I2C1>) -> ! {
    i2c.run().await
}

#[embassy_executor::task]
async fn busy_task() -> ! {
    loop {
        embassy_futures::yield_now().await;
    }
}

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
