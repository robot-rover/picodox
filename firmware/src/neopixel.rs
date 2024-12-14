use embassy_rp::{
    clocks,
    dma::AnyChannel,
    pio::{Config, FifoJoin, Instance, Pio, PioPin, ShiftConfig, ShiftDirection, StateMachine},
    Peripheral, PeripheralRef,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::Timer;
use fixed::types::U24F8;
use pio::{Assembler, JmpCondition, OutDestination, SetDestination};

mod timing {
    pub const T1: u8 = 2; // start bit
    pub const T2: u8 = 5; // data bit
    pub const T3: u8 = 3; // stop bit
    pub const CYCLES_PER_BIT: u32 = (T1 + T2 + T3) as u32;
}

fn assemble_program() -> pio::Program<32> {
    let side_set = pio::SideSet::new(false, 1, false);
    let mut a: pio::Assembler<32> = Assembler::new_with_side_set(side_set);

    let mut wrap_target = a.label();
    let mut wrap_source = a.label();
    let mut do_zero = a.label();
    a.set_with_side_set(SetDestination::PINDIRS, 1, 0);
    a.bind(&mut wrap_target);
    // Do stop bit
    a.out_with_delay_and_side_set(OutDestination::X, 1, timing::T3 - 1, 0);
    // Do start bit
    a.jmp_with_delay_and_side_set(JmpCondition::XIsZero, &mut do_zero, timing::T1 - 1, 1);
    // Do data bit = 1
    a.jmp_with_delay_and_side_set(JmpCondition::Always, &mut wrap_target, timing::T2 - 1, 1);
    a.bind(&mut do_zero);
    // Do data bit = 0
    a.nop_with_delay_and_side_set(timing::T2 - 1, 0);
    a.bind(&mut wrap_source);

    a.assemble_with_wrap(wrap_source, wrap_target)
}

pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }

    pub fn wheel(mut wheel_pos: u8) -> Self {
        wheel_pos = 255 - wheel_pos;
        if wheel_pos < 85 {
            Self::new(255 - wheel_pos * 3, 0, wheel_pos * 3)
        } else if wheel_pos < 170 {
            wheel_pos -= 85;
            Self::new(0, wheel_pos * 3, 255 - wheel_pos * 3)
        } else {
            wheel_pos -= 170;
            Self::new(wheel_pos * 3, 255 - wheel_pos * 3, 0)
        }
    }
}

impl From<Color> for u32 {
    fn from(color: Color) -> Self {
        (u32::from(color.g) << 24) | (u32::from(color.r) << 16) | (u32::from(color.b) << 8)
    }
}

pub struct Neopixel<'d, P: Instance> {
    dma: PeripheralRef<'d, AnyChannel>,
    sm: StateMachine<'d, P, 0>,
    signal: &'d Signal<CriticalSectionRawMutex, Color>,
}

impl<'d, P: Instance> Neopixel<'d, P> {
    pub fn new(
        pio: Pio<'d, P>,
        sig_pin: impl PioPin,
        dma: impl Peripheral<P = AnyChannel> + 'd,
        color_signal: &'d Signal<CriticalSectionRawMutex, Color>,
    ) -> Self {
        let Pio {
            mut common,
            mut sm0,
            ..
        } = pio;

        let prg = assemble_program();
        let mut cfg = Config::default();

        // Pin config
        let out_pin = common.make_pio_pin(sig_pin);
        cfg.set_out_pins(&[&out_pin]);
        cfg.set_set_pins(&[&out_pin]);

        cfg.use_program(&common.load_program(&prg), &[&out_pin]);

        // Clock Config
        // Both _freq values are in kHz
        let clock_freq = U24F8::from_num(clocks::clk_sys_freq() / 1000);
        let ws2812_freq = U24F8::from_num(800);
        let bit_freq = ws2812_freq * timing::CYCLES_PER_BIT;
        cfg.clock_divider = clock_freq / bit_freq;

        // FIFO Config
        cfg.fifo_join = FifoJoin::TxOnly;
        cfg.shift_out = ShiftConfig {
            auto_fill: true,
            threshold: 24,
            direction: ShiftDirection::Left,
        };

        sm0.set_config(&cfg);
        sm0.set_enable(true);

        //let pin = ;
        Neopixel {
            sm: sm0,
            dma: dma.into_ref(),
            signal: color_signal,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let words: [u32; 1] = [self.signal.wait().await.into()];
            self.sm.tx().dma_push(self.dma.reborrow(), &words).await;
            Timer::after_micros(55).await;
        }
    }
}
