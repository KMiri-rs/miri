//@compile-flags: -Zkmiri-toml=dev/test_utils/no-paging.toml

fn main() {
    global();
    local();
}

fn global() {
    static mut BYTES: [u8; 8] = [0; 8];
    let base = (&raw mut BYTES) as *mut u8;
    unsafe {
        let ptr = base.add(3);
        ptr.write(23);
        assert_eq!(ptr.read(), 23);
    }
}

fn local() {
    let mut bytes = [0u8; 8];
    let base = (&raw mut bytes) as *mut u8;
    unsafe {
        let ptr = base.add(3);
        ptr.write(42);
        assert_eq!(ptr.read(), 42);
    }
}
