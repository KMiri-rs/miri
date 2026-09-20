fn main() {
    f();
}

fn f() -> (usize, String) {
    (0, String::new()).clone()
}
