use std::hint::black_box;
use std::mem::MaybeUninit;

fn main() {
    let byte = MaybeUninit::<u8>::uninit();
    black_box(unsafe { byte.assume_init() }); //~ ERROR: uninitialized
}
