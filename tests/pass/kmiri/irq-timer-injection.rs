//@compile-flags: -Zkmiri-toml=dev/test_utils/irq-timer.toml

#[path = "../../../dev/test_utils/irq.rs"]
mod irq;

use std::sync::atomic::{AtomicUsize, Ordering};

static TIMER_RUNS: AtomicUsize = AtomicUsize::new(0);

extern "Rust" fn timer_handler() {
    TIMER_RUNS.fetch_add(1, Ordering::SeqCst);
}

fn main() {
    irq::register_timer_irq(timer_handler);
    while TIMER_RUNS.load(Ordering::SeqCst) == 0 {
        std::hint::spin_loop();
    }
    assert_eq!(TIMER_RUNS.load(Ordering::SeqCst), 1);
}
