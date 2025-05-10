use core::{
    fmt::{self, Write},
    panic::PanicInfo,
    ptr::addr_of_mut,
    sync::atomic::{AtomicUsize, Ordering},
};

use cortex_m_rt::{exception, ExceptionFrame};
use embassy_rp::rom_data;

const BUFFER_SIZE: usize = 1024;

#[no_mangle]
static mut PANIC_BUFFER: [u8; BUFFER_SIZE] = [0u8; BUFFER_SIZE];

struct PanicBuffer<'a> {
    buf: &'a mut [u8],
    offset: usize,
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

    loop {}
}

#[exception]
unsafe fn DefaultHandler(irqn: i16) -> ! {
    panic!("ARM Exception! IRQn: {}", irqn);
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault! {:#?}", ef);
}

#[no_mangle]
static mut TRACE_BUFFER: [u8; BUFFER_SIZE] = [0u8; BUFFER_SIZE];
static TRACE_OFFSET: AtomicUsize = AtomicUsize::new(0);

struct TraceBuffer;

impl<'a> fmt::Write for TraceBuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut s = s;
        let mut offset = critical_section::with(|_| {
            let offset = TRACE_OFFSET.load(Ordering::SeqCst);
            TRACE_OFFSET.store((offset + s.len()) % BUFFER_SIZE, Ordering::SeqCst);
            offset
        });
        while s.len() > 0 {
            let remaining = BUFFER_SIZE - offset;
            let take = s.len().min(remaining);
            let write_part = &s[..take];
            unsafe {
                TRACE_BUFFER[offset..(offset + take)].clone_from_slice(write_part.as_bytes())
            };

            offset += take;
            s = &s[take..];

            if offset == BUFFER_SIZE {
                offset = 0;
            }
        }
        unsafe { TRACE_BUFFER[offset] = '@' as u8 };

        Ok(())
    }
}

#[no_mangle]
fn _embassy_trace_task_new(executor_id: u32, task_id: u32) {
    let mut tb = TraceBuffer;
    let _ = writeln!(tb, "{{NT|exid:{},tid:{}}}", executor_id, task_id);
}

#[no_mangle]
fn _embassy_trace_task_exec_begin(executor_id: u32, task_id: u32) {
    let mut tb = TraceBuffer;
    let _ = writeln!(tb, "{{TB|exid:{},tid:{}}}", executor_id, task_id);
}

#[no_mangle]
fn _embassy_trace_task_exec_end(executor_id: u32, task_id: u32) {
    let mut tb = TraceBuffer;
    let _ = writeln!(tb, "{{TE|exid:{},tid:{}}}", executor_id, task_id);
}

#[no_mangle]
fn _embassy_trace_task_ready_begin(executor_id: u32, task_id: u32) {
    let mut tb = TraceBuffer;
    let _ = writeln!(tb, "{{TR|exid:{},tid:{}}}", executor_id, task_id);
}

#[no_mangle]
fn _embassy_trace_executor_idle(executor_id: u32) {
    let mut tb = TraceBuffer;
    let _ = writeln!(tb, "{{EI|exid:{}}}", executor_id);
}
