//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! A physically overlapping cross-boundary copy leaves the tail of the
//! destination uninitialized; reading that tail is UB.
//!
//!   copy(dst = SRC + 4096, src = SRC + 4090, len = 16)   (dst = src + 6)
//!
//!   SRC page 0 (zeroed)            SRC page 1 (unwritten)
//!   ┌────────────────────────┐     ┌────────────────────┐
//!   │ ... 4090 .. 4095       │     │ 0 .. 9 ...         │
//!   └────────────────────────┘     └────────────────────┘
//!     src bytes 0..5   init: yes      src bytes 6..15  init: no
//!
//!   dst after copy (SRC + 4096 .. SRC + 4111):
//!     byte 0..5   <- page 0   initialized
//!     byte 6..15  <- page 1   uninitialized
//!
//! Reading `SRC + 4096 + 6` (dst byte 6) reads an uninitialized value and is UB.
//! (Mirror of `physical-copy-overlap-valid`, which reads byte 0 instead.)

use std::hint::black_box;
use std::ptr;

#[path = "../../dev/test_utils/physical_copy.rs"]
mod utils;

use utils::*;

fn main() {
    unsafe {
        prepare_paging();
        kern_miri_alloc_pages(SRC, 2);
        kern_miri_zero(SRC, 1);
        kern_miri_copy_untyped(LINEAR + SRC + 4096, LINEAR + SRC + 4090, 16);
        kern_miri_retype_pages(SRC, 2, TypedKind::Slab, 1);
        // Byte 6 came from the uninitialized second source page.
        let p = ptr::with_exposed_provenance::<u8>(LINEAR + SRC + 4096 + 6);
        black_box(p.read()); //~ ERROR: uninitialized
    }
}
