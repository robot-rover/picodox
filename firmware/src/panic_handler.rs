use core::{fmt::{self, Write}, panic::PanicInfo, ptr::addr_of_mut};

use embassy_rp::rom_data;

const PANIC_BUFFER_SIZE: usize = 1024;

#[no_mangle]
static mut PANIC_BUFFER: [u8; PANIC_BUFFER_SIZE] = [0u8; PANIC_BUFFER_SIZE];

struct PanicBuffer<'a> {
    buf: &'a mut [u8],
    offset: usize
}

impl<'a> fmt::Write for PanicBuffer<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.offset + s.len() <= self.buf.len() {
            self.buf[self.offset..self.offset + s.len()].copy_from_slice(s.as_bytes());
            self.offset += s.len();
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

#[inline(never)]
#[panic_handler]
fn panic_handler(panic_info: &PanicInfo<'_>) -> ! {
    let mut buffer = PanicBuffer {
        buf: unsafe { &mut *addr_of_mut!(PANIC_BUFFER) },
        offset: 0,
    };
    //for i in 0u8..=255 {
    //    buffer[i as usize] = i;
    //}
    //const TEST_STR: &str = "Hello world!\r\n";
    //buffer[..TEST_STR.len()].clone_from_slice(TEST_STR.as_bytes());
    let _ = writeln!(buffer, "Panic: {:#}", panic_info);
    rom_data::reset_to_usb_boot(0, 0);

    loop { }
}
