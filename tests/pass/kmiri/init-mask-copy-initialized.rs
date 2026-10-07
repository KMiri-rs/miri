use std::hint::black_box;
use std::mem::MaybeUninit;
use std::ptr;

fn main() {
    let src = MaybeUninit::new(0u8);
    let mut dst = MaybeUninit::uninit();
    unsafe {
        ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), 1);
        assert_eq!(black_box(dst.assume_init()), 0);
    }
}
