use std::sync::atomic::{AtomicBool, Ordering};

static FINISHED: AtomicBool = AtomicBool::new(false);
static mut VALUE: u8 = 0;

fn main() {
    // Kernel-task handoff must not make an ordinary thread's return an
    // implicit join. A relaxed flag orders execution but does not synchronize
    // the non-atomic accesses; this test must still report a data race.
    let thread = std::thread::spawn(|| {
        unsafe { VALUE = 1 };
        FINISHED.store(true, Ordering::Relaxed);
    });
    while !FINISHED.load(Ordering::Relaxed) {
        std::thread::yield_now();
    }
    thread.join().unwrap();
    std::hint::black_box(unsafe { VALUE });
}
