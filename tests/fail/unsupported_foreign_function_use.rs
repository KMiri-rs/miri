fn main() {
    unsafe extern "C" {
        fn foo();
    }

    let _ = foo; //~ ERROR: unsupported operation: can't use unknown foreign function symbol `foo`
}
