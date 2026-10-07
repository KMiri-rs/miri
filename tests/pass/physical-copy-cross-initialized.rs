//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! A copy crossing the page boundary succeeds when both source pages are
//! initialized.
//!
//!   src = SRC + 4090 .. SRC + 4105 (16 B)     dst = DST + 4090
//!
//!   SRC page 0 (zeroed)            SRC page 1 (zeroed)
//!   ┌────────────────────────┐     ┌────────────────────┐
//!   │ ... 4090 .. 4095       │     │ 0 .. 9 ...         │
//!   └────────────────────────┘     └────────────────────┘
//!     src bytes 0..5   init: yes      src bytes 6..15  init: yes
//!
//! Both source pages were zeroed, so all 16 copied bytes land initialized.
//! Reading `DST + 4090` (the first copied byte) is valid.

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
        kern_miri_zero(SRC, 2);
        kern_miri_copy_untyped(LINEAR + DST + 4090, LINEAR + SRC + 4090, 16);
        kern_miri_retype_pages(DST, 2, TypedKind::Slab, 1);
        let p = ptr::with_exposed_provenance::<u8>(LINEAR + DST + 4090);
        assert_eq!(black_box(p.read()), 0);
    }
}
