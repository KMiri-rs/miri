//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! Reading a byte copied from an uninitialized source page is UB.
//!
//!   SRC (never written)           DST (zeroed)
//!   ┌──────────────┐              ┌──────────────┐
//!   │ ?            │  -- copy 1B > │ ?            │
//!   └──────────────┘              └──────────────┘
//!     byte 0: uninitialized          byte 0: uninitialized (copied)
//!
//! The copy transfers the source's uninitialized state, so after retyping `DST`
//! to `Slab`, reading byte 0 is reading uninitialized memory.

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
        kern_miri_zero(DST, 1); // source stays uninitialized
        kern_miri_copy_untyped(LINEAR + DST, LINEAR + SRC, 1);
        kern_miri_retype_pages(DST, 1, TypedKind::Slab, 1);
        let p = ptr::with_exposed_provenance::<u8>(LINEAR + DST);
        black_box(p.read()); //~ ERROR: uninitialized
    }
}
