//@normalize-stderr-test: "OS `.*`" -> "$$OS"

fn main() {
    unsafe extern "Rust" {
        fn foo();
    }

    let _ = foo as *const () as usize; //~ ERROR: unsupported operation: can't use unknown foreign function symbol `foo`
}
