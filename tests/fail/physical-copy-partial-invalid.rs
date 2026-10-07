//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! With only the first source page initialized, reading a byte copied from the
//! second (uninitialized) page is UB.
//!
//!   src = SRC + 4090 .. SRC + 4105 (16 B)     dst = DST + 4090
//!
//!   SRC page 0 (zeroed)            SRC page 1 (unwritten)
//!   ┌────────────────────────┐     ┌────────────────────┐
//!   │ ... 4090 .. 4095       │     │ 0 .. 9 ...         │
//!   └────────────────────────┘     └────────────────────┘
//!     src bytes 0..5   init: yes      src bytes 6..15  init: no
//!
//! Bytes 0..5 of the destination are initialized (copied from page 0), but
//! bytes 6..15 are uninitialized (copied from page 1). Reading `DST + 4090 + 6`
//! (byte 6) reads uninitialized memory.

use std::hint::black_box;
use std::ptr;

#[path = "../../dev/test_utils/physical_copy.rs"]
mod utils;

use utils::*;

fn main() {
    unsafe {
        prepare_paging();
        kern_miri_alloc_pages(SRC, 2);
        kern_miri_alloc_pages(DST, 2);
        kern_miri_zero(SRC, 1); // only the first source page is initialized
        kern_miri_copy_untyped(LINEAR + DST + 4090, LINEAR + SRC + 4090, 16);
        kern_miri_retype_pages(DST, 2, TypedKind::Slab, 1);
        // Byte 6 came from the uninitialized second source page.
        let p = ptr::with_exposed_provenance::<u8>(LINEAR + DST + 4090 + 6);
        black_box(p.read()); //~ ERROR: uninitialized
    }
}
