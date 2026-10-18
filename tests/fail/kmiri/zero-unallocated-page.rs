//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml

//! Zeroing a page that was never handed to `kern_miri_alloc_pages` is UB.
//!
//! Only `DST` is allocated. The range `DST..DST + 2 pages` also covers the next,
//! unallocated page, so the shim must report it instead of panicking the
//! interpreter. The check runs before any byte is written.

#[path = "../../../dev/test_utils/physical_copy.rs"]
mod utils;

use utils::*;

fn main() {
    unsafe {
        kern_miri_alloc_pages(DST, 1);
        kern_miri_zero(DST, 2); //~ ERROR: not allocated by kern_miri_alloc_pages
    }
}
