use std::hint::black_box;
use std::mem::MaybeUninit;
use std::ptr;

fn main() {
    let src = [MaybeUninit::new(7u8), MaybeUninit::uninit()];
    let mut dst = [MaybeUninit::new(0u8); 2];
    unsafe {
        ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), 2);
        assert_eq!(black_box(dst[0].assume_init()), 7);
    }
}
