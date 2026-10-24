//@compile-flags: -Zkmiri-toml=dev/test_utils/no-paging.toml

use std::sync::atomic::{AtomicUsize, Ordering};

static FIRST: u8 = 1;
static SECOND: u8 = 2;
static PHASE: AtomicUsize = AtomicUsize::new(0);

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

struct Finish;
struct FinishTls;

thread_local! {
    static TLS: FinishTls = const { FinishTls };
}

impl Drop for Finish {
    fn drop(&mut self) {
        assert_eq!(PHASE.swap(2, Ordering::SeqCst), 1);
    }
}

impl Drop for FinishTls {
    fn drop(&mut self) {
        assert_eq!(PHASE.swap(3, Ordering::SeqCst), 2);
    }
}

fn first(_: usize) {
    let _finish = Finish;
    TLS.with(|_| {});
    // The successor must wait for both the entry's Drop and its TLS destructors.
    unsafe { miri_switch_to_after_return(&SECOND) };
    assert_eq!(PHASE.swap(1, Ordering::SeqCst), 0);
}

fn second(_: usize) {
    assert_eq!(PHASE.swap(4, Ordering::SeqCst), 3);
}

fn main() {
    // Distinct task tokens and non-overlapping stacks model two kernel tasks.
    // Both entries must finish; suppressing thread/leak checks would hide the bug.
    unsafe {
        miri_create_new_thread(first, 0, &FIRST, 0x7000000, 0x100000);
        miri_create_new_thread(second, 0, &SECOND, 0x7200000, 0x100000);
        miri_switch_to(&FIRST);
    }
    assert_eq!(PHASE.load(Ordering::SeqCst), 4);
}
