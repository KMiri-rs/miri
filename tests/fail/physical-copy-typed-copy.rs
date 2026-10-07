//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! `kern_miri_copy_untyped` requires untyped pages: retyping the source to
//! `Slab` first makes the copy unsupported.
//!
//!   SRC (zeroed, retyped Slab)      DST (untyped)
//!   ┌──────────────┐                ┌──────────────┐
//!   │ 7            │  -- copy 1B X > │              │
//!   └──────────────┘                └──────────────┘
//!     typed: Slab                      (never typed)
//!
//! An untyped copy is only defined over exclusively untyped pages, so copying
//! out of a typed page is reported as an unsupported operation.

use std::ptr;

#[path = "../../dev/test_utils/physical_copy.rs"]
mod utils;

use utils::*;

fn main() {
    unsafe {
        prepare_paging();
        kern_miri_alloc_pages(SRC, 1);
        kern_miri_alloc_pages(DST, 1);
        kern_miri_zero(SRC, 1);
        kern_miri_retype_pages(SRC, 1, TypedKind::Slab, 1);
        let p = ptr::with_exposed_provenance_mut::<u8>(LINEAR + SRC);
        p.write(7);
        kern_miri_copy_untyped(LINEAR + DST, LINEAR + SRC, 1); //~ ERROR: untyped physical copy
    }
}
