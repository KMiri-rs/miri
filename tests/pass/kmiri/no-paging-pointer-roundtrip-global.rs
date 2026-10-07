//@compile-flags: -Zkmiri-toml=dev/test_utils/no-paging.toml -Zmiri-permissive-provenance

fn main() {
    static mut BYTES: [u8; 8] = [0; 8];
    let base = &raw mut BYTES;
    unsafe {
        let ptr = (base as usize as *mut u8).add(3);
        ptr.write(23);
        assert_eq!(ptr.read(), 23);
    }
}
