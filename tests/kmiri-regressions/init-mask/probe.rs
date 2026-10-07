#![no_std]
#![no_main]
use core::{mem::MaybeUninit, ptr};
unsafe extern "Rust" { fn miri_write_to_stdout(bytes: &[u8]); }
unsafe extern "C" { fn abort() -> !; }
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! { unsafe { abort() } }

static UNKNOWN: MaybeUninit<u8> = MaybeUninit::uninit();
#[unsafe(no_mangle)]
fn miri_start(_: isize, _: *const *const u8) -> isize {
    unsafe { miri_write_to_stdout(b"G0_ENTRY\n") };
    if cfg!(probe = "global_uninit") {
        core::hint::black_box(unsafe { UNKNOWN.assume_init() });
    } else if cfg!(probe = "direct_uninit") {
        let byte = MaybeUninit::<u8>::uninit();
        core::hint::black_box(unsafe { byte.assume_init() });
    } else if cfg!(probe = "copy_initialized") || cfg!(probe = "copy_uninitialized")
        || cfg!(probe = "copy_uninitialized_no_read") {
        let (src, mut dst) = if cfg!(probe = "copy_initialized") {
            (MaybeUninit::new(0u8), MaybeUninit::uninit())
        } else {
            (MaybeUninit::uninit(), MaybeUninit::new(0u8))
        };
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), 1);
            if !cfg!(probe = "copy_uninitialized_no_read") {
                assert_eq!(dst.assume_init(), 0);
            }
        }
    } else if cfg!(probe = "partial_valid") || cfg!(probe = "partial_invalid") {
        let src = [MaybeUninit::new(7u8), MaybeUninit::uninit()];
        let mut dst = [MaybeUninit::new(0u8); 2];
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), 2);
            let index = usize::from(cfg!(probe = "partial_invalid"));
            let value = dst[index].assume_init();
            core::hint::black_box(value);
            if index == 0 { assert_eq!(value, 7); }
        }
    } else {
        panic!("unknown probe");
    }
    unsafe { miri_write_to_stdout(b"G0_DONE\n") };
    0
}
