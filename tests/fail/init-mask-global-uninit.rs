use std::hint::black_box;
use std::mem::MaybeUninit;

static UNKNOWN: MaybeUninit<u8> = MaybeUninit::uninit();

fn main() {
    black_box(unsafe { UNKNOWN.assume_init() }); //~ ERROR: uninitialized
}
