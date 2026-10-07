//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! The simplest copy: one initialized byte between two untyped pages.
//!
//!   SRC (1 page, zeroed)          DST (1 page, untyped)
//!   ┌──────────────┐              ┌──────────────┐
//!   │ 0            │  -- copy 1B > │ 0            │
//!   └──────────────┘              └──────────────┘
//!     byte 0: initialized           byte 0: initialized (copied)
//!
//! `kern_miri_copy_untyped` transfers the source's per-byte init state, so the
//! destination byte is initialized. Retyping `DST` to `Slab` and reading byte 0
//! succeeds and yields 0.

use std::hint::black_box;
use std::ptr;

#[path = "../../../dev/test_utils/physical_copy.rs"]
mod utils;

use utils::*;

fn main() {
    unsafe {
        prepare_paging();
        kern_miri_alloc_pages(SRC, 1);
        kern_miri_alloc_pages(DST, 1);
        kern_miri_zero(SRC, 1);
        kern_miri_copy_untyped(LINEAR + DST, LINEAR + SRC, 1);
        kern_miri_retype_pages(DST, 1, TypedKind::Slab, 1);
        let p = ptr::with_exposed_provenance::<u8>(LINEAR + DST);
        assert_eq!(black_box(p.read()), 0);
    }
}
