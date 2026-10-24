//! `unaligned_volatile_load` reads a value through a pointer that is not aligned
//! for its type. Asterinas' `VmReader::read_val` uses it on arbitrary byte offsets.
//! The `u32` is read from offset 1 of a byte buffer, so the pointer is misaligned.

#![feature(core_intrinsics)]
#![allow(internal_features)]

use std::intrinsics::unaligned_volatile_load;

fn main() {
    let bytes: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
    let ptr = bytes.as_ptr().wrapping_add(1).cast::<u32>();
    let val = unsafe { unaligned_volatile_load(ptr) };
    assert_eq!(val, u32::from_ne_bytes([1, 2, 3, 4]));
}
