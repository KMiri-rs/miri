//@compile-flags: -Zkmiri-toml=dev/test_utils/no-paging.toml

use std::sync::atomic::{AtomicUsize, Ordering};

static TASKS: [u8; 3] = [1, 2, 3];
static FINISHED: AtomicUsize = AtomicUsize::new(0);

unsafe extern "Rust" {
    fn miri_create_new_thread(
        func: fn(usize),
        arg: usize,
        task: &u8,
        stack_end: usize,
        stack_size: usize,
    );
    fn miri_switch_to(task: &u8);
    fn miri_switch_to_after_return(task: &u8);
}

fn entry(index: usize) {
    assert_eq!(FINISHED.load(Ordering::SeqCst), index);
    if index + 1 < TASKS.len() {
        // Each exiting task hands off only after its own entry returns.
        unsafe { miri_switch_to_after_return(&TASKS[index + 1]) };
    }
    assert_eq!(FINISHED.fetch_add(1, Ordering::SeqCst), index);
}

fn main() {
    // A chain catches stale handoff state; the final task has no successor.
    unsafe {
        for index in 0..TASKS.len() {
            miri_create_new_thread(
                entry,
                index,
                &TASKS[index],
                0x7000000 + index * 0x200000,
                0x100000,
            );
        }
        miri_switch_to(&TASKS[0]);
    }
    assert_eq!(FINISHED.load(Ordering::SeqCst), TASKS.len());
}
