//@compile-flags: -Zkmiri-toml=dev/test_utils/no-paging.toml -Zmiri-permissive-provenance

fn main() {
    let mut bytes = [0u8; 8];
    let base = (&raw mut bytes) as *mut u8;
    unsafe {
        let ptr = (base as usize as *mut u8).add(3);
        ptr.write(42);
        assert_eq!(ptr.read(), 42);
    }
}
