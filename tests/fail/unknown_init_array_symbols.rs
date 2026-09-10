//! Linker-defined init-array symbols must not get fake addresses that can be
//! subtracted (that overflowed in Asterinas/OSTD). KMiri#76.
//@normalize-stderr-test: "OS `.*`" -> "$$OS"

fn main() {
    unsafe extern "C" {
        fn __sinit_array();
        fn __einit_array();
    }

    // Mimic `ostd::invoke_ffi_init_funcs`: treat the two symbols as a function
    // pointer table. This must error on the unknown symbols, not overflow.
    let end = __einit_array as *const () as usize; //~ ERROR: unsupported operation: can't use unknown foreign function symbol `__einit_array`
    let start = __sinit_array as *const () as usize; //~ ERROR: unsupported operation: can't use unknown foreign function symbol `__sinit_array`
    let _ = end.wrapping_sub(start);
}
