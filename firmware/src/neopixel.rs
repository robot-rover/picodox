
use embassy_rp::{dma, pio::{self, Pio, PioPin}, Peripheral};
use smart_leds::RGB8;

use crate::pio_ws2812::{PioWs2812, PioWs2812Program};

pub struct Neopixel<'d, P: pio::Instance, S: PioPin> {
    program: PioWs2812Program<'d, P>,
    ws2812: PioWs2812,
    data: [RGB8; 1],
}

impl Neopixel {
    pub fn new<'d>(pio: Pio<'d, impl pio::Instance>, sig_pin: impl PioPin, dma: impl Peripheral<P = impl dma::Channel> + 'd) -> Self {
        let Pio { mut common, sm0, .. } = pio;
        //let pin = p.PIN_17;
        let program = PioWs2812Program::new(&mut common);
        let ws2812 = PioWs2812::new(&mut common, sm0, dma, sig_pin, &program);
        Neopixel {
            data: (0, 0, 0).into(),
        }
    }

}

