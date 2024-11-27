
use core::{cell::RefCell, slice, sync::atomic::Ordering};

use critical_section;
use circular_buffer::CircularBuffer;
use embassy_futures::select::{select, Either};
use embassy_sync::{blocking_mutex::{raw::CriticalSectionRawMutex, Mutex}, signal::Signal};
use embassy_usb::{class::cdc_acm::{CdcAcmClass, State}, driver::Driver, Builder};
use portable_atomic::AtomicBool;

const MAX_PACKET_SIZE: usize = 64;

struct LoggerComs {
    buf: Mutex<CriticalSectionRawMutex, RefCell<CircularBuffer< { 2*MAX_PACKET_SIZE }, u8>>>,
    sig: Signal<CriticalSectionRawMutex, ()>,
}

pub static GLOBAL_COMS: LoggerComs = LoggerComs {
    buf: Mutex::new(RefCell::new(CircularBuffer::new())),
    sig: Signal::new(),
};

/// Global logger lock.
static TAKEN: AtomicBool = AtomicBool::new(false);
static mut CS_RESTORE: critical_section::RestoreState = critical_section::RestoreState::invalid();
static mut ENCODER: defmt::Encoder = defmt::Encoder::new();

unsafe impl defmt::Logger for LoggerComs {
    fn acquire() {
        // safety: Must be paired with corresponding call to release(), see below
        let restore = unsafe { critical_section::acquire() };

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        if TAKEN.load(Ordering::Relaxed) {
            panic!("defmt logger taken reentrantly")
        }

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        TAKEN.store(true, Ordering::Relaxed);

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        unsafe { CS_RESTORE = restore };

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        unsafe { ENCODER.start_frame(do_write) }
    }

    unsafe fn flush() {
        // safety: accessing the `&'static _` is OK because we have acquired a critical section.
        flush();
    }

    unsafe fn release() {
        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        ENCODER.end_frame(do_write);

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        TAKEN.store(false, Ordering::Relaxed);

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        let restore = CS_RESTORE;

        // safety: Must be paired with corresponding call to acquire(), see above
        critical_section::release(restore);
    }

    unsafe fn write(bytes: &[u8]) {
        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        ENCODER.write(bytes, do_write);
    }
}

fn do_write(bytes: &[u8]) {
    let fullness = GLOBAL_COMS.buf.lock(|buf_cell| {
        let mut buf = buf_cell.borrow_mut();
        buf.extend_from_slice(bytes);
        buf.len()
    });

    if fullness > MAX_PACKET_SIZE {
        flush()
    }
}

fn flush() {
    GLOBAL_COMS.sig.signal(());
}

pub struct LoggerIf<'d, D>
where D: Driver<'d> {
    // TODO: Since reading has no dependence, should split to its own task
    class: CdcAcmClass<'d, D>,
    send_buf: [u8; MAX_PACKET_SIZE],
    recv_buf: [u8; MAX_PACKET_SIZE],
}

impl<'d, D: Driver<'d>> LoggerIf<'d, D> {
    async fn wait_for_data(&mut self) {
        loop {
            match select(self.class.read_packet(&mut self.recv_buf), GLOBAL_COMS.sig.wait()).await {
                // TODO: Log errors here
                Either::First(_read_count) => {},
                Either::Second(_unit) => return,
            };
        };
    }

    pub async fn run(&mut self) {
        loop {
            self.wait_for_data().await;

            let (is_all, send_len) = GLOBAL_COMS.buf.lock(|buf_cell| {
                let mut buf = buf_cell.borrow_mut();
                let take_count = buf.len().min(MAX_PACKET_SIZE);
                let is_all = take_count == buf.len();
                for (idx, byte) in buf.drain(..take_count).enumerate() {
                    self.send_buf[idx] = byte;
                }
                (is_all, take_count)
            });

            self.class.write_packet(&self.send_buf[..send_len]).await;

            // Add the ZLP to flush buffer if no more data is waiting
            if is_all
                && send_len == MAX_PACKET_SIZE
                && GLOBAL_COMS.buf.lock(|buf_cell| buf_cell.borrow().is_empty())
            {
                self.class.write_packet(&[]).await;
            }
        }
    }

    pub fn setup(builder: &mut Builder<'d, D>, state: &'d mut State<'d>) -> Self {
        let class = CdcAcmClass::new(builder, state, MAX_PACKET_SIZE as u16);

        LoggerIf {
            class,
            recv_buf: [0u8; MAX_PACKET_SIZE],
            send_buf: [0u8; MAX_PACKET_SIZE],
        }
    }

}

