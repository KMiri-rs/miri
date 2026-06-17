#![feature(linkage)]

// `__GLOBAL_HEAP_ALLOCATOR_REF` and `__GLOBAL_FRAME_ALLOCATOR_REF` statics are specially handled;
// other statics may be flagged "extern static `...` is not supported by Miri".
mod first {
    #[no_mangle]
    #[linkage = "weak"]
    static __GLOBAL_HEAP_ALLOCATOR_REF: i32 = 1;

    #[no_mangle]
    #[linkage = "weak"]
    static __GLOBAL_FRAME_ALLOCATOR_REF: i32 = 2;
}

mod second {
    #[no_mangle]
    static __GLOBAL_FRAME_ALLOCATOR_REF: i32 = 3;
}

unsafe extern "Rust" {
    static __GLOBAL_HEAP_ALLOCATOR_REF: i32;
    static __GLOBAL_FRAME_ALLOCATOR_REF: i32;
}

fn main() {
    unsafe {
        // If there is no non-weak definition, the weak definition will be used.
        assert_eq!(__GLOBAL_HEAP_ALLOCATOR_REF, 1);
        // Non-weak definition takes precedence over a weak definition.
        assert_eq!(__GLOBAL_FRAME_ALLOCATOR_REF, 3);
    }
}
