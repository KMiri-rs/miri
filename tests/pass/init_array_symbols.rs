//@compile-flags: -Zkmiri-toml=tests/pass/init_array_symbols.toml

//! Unknown `extern` symbols are rejected by default. Mapping them in
//! `kmiri.toml` `[layout]` is the shim that keeps their addresses (KMiri#76).

fn main() {
    unsafe extern "C" {
        fn __sinit_array();
        fn __einit_array();
    }

    let start = __sinit_array as *const () as usize;
    let end = __einit_array as *const () as usize;
    assert_eq!(start, 0x80014000);
    assert_eq!(end, 0x80014010);
    assert_eq!((end - start) / 8, 2);
}
