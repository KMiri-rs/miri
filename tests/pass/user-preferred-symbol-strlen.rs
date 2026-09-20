//@compile-flags: -Zkmiri-toml=tests/pass/user-preferred-symbol-strlen.toml

//! When `[user_preferred_symbol]` names a symbol, calls to that symbol must
//! execute the user's own definition rather than the built-in shim, regardless
//! of how the call is made:
//! - direct `extern "C"` call,
//! - indirect call through a function pointer, or
//! - indirect call from another foreign function that internally calls strlen.
//! Regression test for <https://github.com/KMiri-rs/KMiri/issues/171>.

use std::ffi::{CStr, c_char};
use std::sync::atomic::{AtomicBool, Ordering};

static USER_STRLEN_CALLED: AtomicBool = AtomicBool::new(false);

/// User-provided strlen override.  Returns a sentinel (99) that the real shim
/// would never produce for any of the strings used below.
#[no_mangle]
unsafe extern "C" fn strlen(_s: *const c_char) -> usize {
    USER_STRLEN_CALLED.store(true, Ordering::SeqCst);
    99
}

// A different Rust name but the same link symbol, so this compiles without a
// name conflict while still routing through `emulate_foreign_item("strlen")`.
unsafe extern "C" {
    #[link_name = "strlen"]
    fn c_strlen(s: *const c_char) -> usize;
}

// User-defined "libc-like" function that has no built-in shim in KMiri, so Miri
// resolves its body via lookup_exported_symbol.  It mirrors how real libc
// functions (e.g. strdup) call strlen internally.  Since there is no shim clash
// for `strdup`, it does NOT need to appear in [user_preferred_symbol].
#[no_mangle]
unsafe extern "C" fn strdup(s: *const c_char) -> *mut c_char {
    c_strlen(s); // internally calls the strlen symbol
    std::ptr::null_mut()
}

unsafe extern "C" {
    #[link_name = "strdup"]
    fn c_strdup(s: *const c_char) -> *mut c_char;
}

fn main() {
    let s = c"hello"; // real length is 5; our override returns 99

    // --- Direct call via extern "C" declaration ---
    let result = unsafe { c_strlen(s.as_ptr()) };
    assert_eq!(result, 99, "direct call: user strlen not called (got {result})");
    assert!(USER_STRLEN_CALLED.swap(false, Ordering::SeqCst), "USER_STRLEN_CALLED not set");

    // --- Indirect call via function pointer (global symbol resolution) ---
    let fn_ptr: unsafe extern "C" fn(*const c_char) -> usize = c_strlen;
    let result = unsafe { fn_ptr(s.as_ptr()) };
    assert_eq!(result, 99, "fn-ptr call: user strlen not called (got {result})");
    assert!(USER_STRLEN_CALLED.swap(false, Ordering::SeqCst), "USER_STRLEN_CALLED not set");

    // --- Indirect call via a foreign function that internally calls strlen ---
    // `strdup` has no KMiri shim, so Miri resolves it to the user body above,
    // which calls `c_strlen` (= strlen).  This simulates a real libc function
    // (like the actual strdup) calling strlen internally.
    let _ptr = unsafe { c_strdup(s.as_ptr()) };
    assert!(USER_STRLEN_CALLED.swap(false, Ordering::SeqCst), "strlen not called via strdup→strlen chain");

    // --- Call via core's CStr::from_ptr (a real stdlib consumer of strlen) ---
    // `CStr::from_ptr` uses `const_eval_select`: at runtime it calls
    // `extern "C" { fn strlen }`, making it the canonical "global dependency"
    // scenario where a standard-library function resolves strlen from the
    // symbol table.
    //
    // Our strlen returns the sentinel 99, so CStr::from_ptr constructs a
    // 100-byte slice.  Use a 128-byte all-zero buffer so that slice stays
    // within the allocation and Miri's reference-validity check passes.
    // We drop the result immediately without reading it.
    let buf = [0u8; 128];
    let _ = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
    assert!(USER_STRLEN_CALLED.load(Ordering::SeqCst), "strlen not called via CStr::from_ptr");
}
