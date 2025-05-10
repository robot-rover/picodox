use core::{cell::RefCell, ptr::addr_of_mut, sync::atomic::Ordering};

use circular_buffer::CircularBuffer;
use critical_section;
use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, Mutex},
    signal::Signal,
};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, Receiver, Sender, State},
    driver::Driver,
    Builder,
};
use portable_atomic::AtomicBool;

const MAX_PACKET_SIZE: usize = 64;

struct LoggerComs {
    buf: Mutex<CriticalSectionRawMutex, RefCell<CircularBuffer<{ 10 * MAX_PACKET_SIZE }, u8>>>,
    sig: Signal<CriticalSectionRawMutex, ()>,
}

static GLOBAL_COMS: LoggerComs = LoggerComs {
    buf: Mutex::new(RefCell::new(CircularBuffer::new())),
    sig: Signal::new(),
};

/// Global logger lock.
static TAKEN: AtomicBool = AtomicBool::new(false);
static mut CS_RESTORE: critical_section::RestoreState = critical_section::RestoreState::invalid();
static mut ENCODER: defmt::Encoder = defmt::Encoder::new();

#[defmt::global_logger]
struct Logger;

unsafe impl defmt::Logger for Logger {
    fn acquire() {
        // safety: Must be paired with corresponding call to release(), see below
        let restore = unsafe { critical_section::acquire() };

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        if TAKEN.load(Ordering::Acquire) {
            panic!("defmt logger taken reentrantly")
        }

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        TAKEN.store(true, Ordering::Release);

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        unsafe { CS_RESTORE = restore };

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        unsafe { (*addr_of_mut!(ENCODER)).start_frame(do_write) }
    }

    unsafe fn flush() {
        // safety: accessing the `&'static _` is OK because we have acquired a critical section.
        flush();
    }

    unsafe fn release() {
        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        (*addr_of_mut!(ENCODER)).end_frame(do_write);

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        TAKEN.store(false, Ordering::Release);

        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        let restore = CS_RESTORE;

        // safety: Must be paired with corresponding call to acquire(), see above
        critical_section::release(restore);
    }

    unsafe fn write(bytes: &[u8]) {
        // safety: accessing the `static mut` is OK because we have acquired a critical section.
        (*addr_of_mut!(ENCODER)).write(bytes, do_write);
    }
}

fn do_write(bytes: &[u8]) {
    let fullness = GLOBAL_COMS.buf.lock(|buf_cell| {
        let mut buf = buf_cell.borrow_mut();
        buf.extend_from_slice(bytes);
        buf.len()
    });

    if fullness > 0 {
        flush()
    }
}

fn flush() {
    GLOBAL_COMS.sig.signal(());
}

pub struct LoggerIf<'d, D: Driver<'d>> {
    sender: Sender<'d, D>,
    send_buf: [u8; MAX_PACKET_SIZE],
}

pub struct LoggerRxSink<'d, D: Driver<'d>> {
    receiver: Receiver<'d, D>,
    recv_buf: [u8; MAX_PACKET_SIZE],
}

impl<'d, D: Driver<'d>> LoggerIf<'d, D> {
    pub async fn run(&mut self) -> ! {
        loop {
            GLOBAL_COMS.sig.wait().await;

            let (is_all, send_len, is_empty) = GLOBAL_COMS.buf.lock(|buf_cell| {
                let mut buf = buf_cell.borrow_mut();
                let take_count = buf.len().min(MAX_PACKET_SIZE);
                let is_all = take_count == buf.len();
                for (idx, byte) in buf.drain(..take_count).enumerate() {
                    self.send_buf[idx] = byte;
                }
                (is_all, take_count, buf.is_empty())
            });

            // Since this is the error reporting mechanism, just fail silently
            let _ = self.sender.write_packet(&self.send_buf[..send_len]).await;

            // Add the ZLP to flush buffer if no more data is waiting
            if is_all && send_len == MAX_PACKET_SIZE && is_empty {
                // Since this is the error reporting mechanism, just fail silently
                let _ = self.sender.write_packet(&[]).await;
            }
        }
    }
}

impl<'d, D: Driver<'d>> LoggerRxSink<'d, D> {
    pub async fn run(&mut self) -> ! {
        loop {
            // TODO: Can we ignore these instead?
            let _ = self.receiver.read_packet(&mut self.recv_buf).await;
        }
    }
}

pub fn new<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
    state: &'d mut State<'d>,
) -> (LoggerIf<'d, D>, LoggerRxSink<'d, D>) {
    let class = CdcAcmClass::new(builder, state, MAX_PACKET_SIZE as u16);
    let (sender, receiver) = class.split();

    (
        LoggerIf {
            sender,
            send_buf: [0u8; MAX_PACKET_SIZE],
        },
        LoggerRxSink {
            receiver,
            recv_buf: [0u8; MAX_PACKET_SIZE],
        },
    )
}
