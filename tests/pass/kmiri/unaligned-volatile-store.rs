//! `unaligned_volatile_store` writes a value through a pointer that is not aligned
//! for its type. Asterinas' `VmWriter::write_val` uses it on arbitrary byte offsets.
//! The `u32` is written at offset 1 of a byte buffer; the bytes around it must stay
//! unchanged.

#![feature(core_intrinsics)]
#![allow(internal_features)]

use std::intrinsics::unaligned_volatile_store;

fn main() {
    let mut bytes = [0u8; 8];
    let ptr = bytes.as_mut_ptr().wrapping_add(1).cast::<u32>();
    unsafe { unaligned_volatile_store(ptr, u32::from_ne_bytes([1, 2, 3, 4])) };
    assert_eq!(bytes, [0, 1, 2, 3, 4, 0, 0, 0]);
}
