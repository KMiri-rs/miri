//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! A cross-boundary copy carries the uninitialized source bytes; reading the
//! first one back is UB.
//!
//!   src = SRC + 4090 .. SRC + 4105 (16 B)     dst = DST + 4090
//!
//!   SRC page 0 (unwritten)         SRC page 1 (unwritten)
//!   ┌────────────────────────┐     ┌────────────────────┐
//!   │ ... 4090 .. 4095       │     │ 0 .. 9 ...         │
//!   └────────────────────────┘     └────────────────────┘
//!     src bytes 0..5   init: no       src bytes 6..15  init: no
//!
//! Both source pages are uninitialized, so every copied byte — including byte 0
//! at `DST + 4090` — stays uninitialized. Reading it is UB.

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
        kern_miri_zero(DST, 2);
        kern_miri_copy_untyped(LINEAR + DST + 4090, LINEAR + SRC + 4090, 16);
        kern_miri_retype_pages(DST, 2, TypedKind::Slab, 1);
        let p = ptr::with_exposed_provenance::<u8>(LINEAR + DST + 4090);
        black_box(p.read()); //~ ERROR: uninitialized
    }
}
