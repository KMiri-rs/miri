//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! Copying from an uninitialized source is fine as long as nothing reads the
//! result.
//!
//!   SRC (never written)           DST (zeroed)
//!   ┌──────────────┐              ┌──────────────┐
//!   │ ?            │  -- copy 1B > │ ?            │
//!   └──────────────┘              └──────────────┘
//!     byte 0: uninitialized          byte 0: uninitialized (copied)
//!
//! The copy preserves the source's uninitialized state; uninitialized data only
//! becomes UB when it is read. This test copies and then stops — no retype, no
//! read — so it is valid.

#[path = "../../../dev/test_utils/physical_copy.rs"]
mod utils;

use utils::*;

fn main() {
    unsafe {
        prepare_paging();
        kern_miri_alloc_pages(SRC, 1);
        kern_miri_alloc_pages(DST, 1);
        kern_miri_zero(DST, 1); // keep the source uninitialized
        kern_miri_copy_untyped(LINEAR + DST, LINEAR + SRC, 1);
        // No retype, no read.
    }
}
