//@compile-flags: -Zkmiri-toml=dev/test_utils/physical-copy.toml -Zmiri-permissive-provenance

//! Copy a single initialized page carrying real byte values (0..=16) and check
//! every byte survives the copy. This exercises a non-zero payload end to end:
//! the source is written through the raw-write shim (pages stay `Untyped`), the
//! copy moves both the bytes and their per-byte init state, and the destination
//! is read back after retyping to `Slab`.

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

        // 17 bytes 0..=16 written straight into the (still Untyped) source page.
        let pattern: [u8; 17] = core::array::from_fn(|i| i as u8);
        kern_miri_write_bytes(SRC, pattern.as_ptr(), pattern.len());

        kern_miri_copy_untyped(LINEAR + DST, LINEAR + SRC, pattern.len());

        kern_miri_retype_pages(DST, 1, TypedKind::Slab, 1);
        for i in 0..pattern.len() {
            let p = ptr::with_exposed_provenance::<u8>(LINEAR + DST + i);
            assert_eq!(black_box(p.read()), pattern[i]);
        }
    }
}
