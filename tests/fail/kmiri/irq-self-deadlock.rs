//@compile-flags: -Zkmiri-toml=dev/test_utils/irq.toml

#[path = "../../../dev/test_utils/irq.rs"]
mod irq;

use std::sync::atomic::{AtomicUsize, Ordering};

const WRITER: usize = 1 << (usize::BITS - 1);
static LOCK: AtomicUsize = AtomicUsize::new(0);

struct ReadGuard;

impl ReadGuard {
    fn acquire() -> Self {
        let old = LOCK.fetch_add(1, Ordering::Acquire);
        assert_eq!(old & WRITER, 0);
        Self
    }
}

impl Drop for ReadGuard {
    fn drop(&mut self) {
        LOCK.fetch_sub(1, Ordering::Release);
    }
}

fn write_lock() {
    loop {
        if LOCK
            .compare_exchange(0, WRITER, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            LOCK.store(0, Ordering::Release);
            return;
        }
        std::hint::spin_loop(); //~ ERROR: IRQ watchdog: possible self-deadlock
    }
}

extern "Rust" fn timer_irq_handler() {
    write_lock();
}

fn main() {
    let _reader = ReadGuard::acquire();
    // The queued handler runs at the next MIR safe point while the reader is held.
    irq::raise_irq(timer_irq_handler);
}
