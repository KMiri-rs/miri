use std::mem::MaybeUninit;
use std::ptr;

fn main() {
    let src = MaybeUninit::<u8>::uninit();
    let mut dst = MaybeUninit::new(0u8);
    unsafe {
        ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), 1);
    }
}
