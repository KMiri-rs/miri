---
name: asterinas-testing
description: Add, update, or run KMiri native tests and Asterinas kernel tests. Use when working with ./tests/pass/kmiri, ./tests/fail/kmiri, ../tests/<case>, cargo osdk miri test, or the asterinas Makefile targets.
---

# Testing KMiri and Asterinas

Follow the path conventions in `./.agents/skills/normalize-local-path-for-skill-contents/SKILL.md`. Paths and commands below are relative to the KMiri repository root unless a different working directory is stated.

## Prefer a focused KMiri integration test for core behavior

When the behavior can be tested without booting an Asterinas kernel, express the core logic as a KMiri integration test. Use the cases in `./tests/pass/kmiri/physical-copy-initialized.rs` and `./tests/fail/kmiri/physical-copy-cross-uninitialized.rs` as examples.

Put every new KMiri integration test only in one of these directories:

- `./tests/pass/kmiri/` for tests expected to pass.
- `./tests/fail/kmiri/` for tests expected to report an error.

Keep each case small and focused. For tests that need shared helpers or a KMiri TOML configuration, put those files under `./dev/` and reference Rust helpers with `#[path]`. For example, the physical-copy tests share `./dev/test_utils/physical_copy.rs` using:

```rust
#[path = "../../../dev/test_utils/physical_copy.rs"]
mod utils;
```

The `#[path]` value is relative to the test source file. Shared test configuration can be selected with a `//@compile-flags` header, as in the physical-copy cases.

From the KMiri repository root, run the matching tests with:

```sh
./miri test kmiri/physical
```

To update expected `.stderr` output after intentionally changing diagnostics, use:

```sh
./miri test --bless kmiri/physical
```

Use the narrowest path filter that covers the changed cases.

## Add an Asterinas kernel test

Use an Asterinas kernel test when the behavior depends on OSTD or requires the kernel test environment. From the KMiri repository root, create a package with OSDK:

```sh
cd ../tests && OSDK_LOCAL_DEV=1 cargo osdk new --kernel my-case
```

Then edit `../tests/my-case/src/lib.rs`. The kernel test entry point should follow this template:

```rust
#![no_std]
#![deny(unsafe_code)]
#![feature(format_args_nl, stmt_expr_attributes)]

extern crate alloc;

#[ostd::ktest::miri_main]
fn kernel_main() {
    // test code
}
```

Keep the test focused on one behavior. Add a comment describing the invariant or regression, and make success or failure observable through an assertion, panic, or distinctive `ostd::miri_println!` message. Use the generated `Cargo.toml` and nearby cases as references rather than hand-writing a package manifest.

## Run one Asterinas test

Prefer running only the changed kernel test. From the KMiri repository root:

```sh
make asterinas ASTERINAS_TEST=my-case
```

The `asterinas` target installs local KMiri and OSDK tools, then runs `cargo osdk miri test` in `../tests/my-case`. `ASTERINAS_TEST` defaults to `init`.

After the tools are installed, the equivalent direct command is:

```sh
cd ../tests/my-case
OSDK_LOCAL_DEV=1 MIRIFLAGS="-Zkmiri-toml=../asterinas.toml -Zmiri-permissive-provenance" cargo osdk miri test
```

The two `MIRIFLAGS` options match `ASTERINAS_ENV_VARS` in `./Makefile` and select the shared Asterinas configuration and permissive provenance mode.

## Run the curated Asterinas suite

`make asterinas-test-all` runs every name in `ASTERINAS_TESTS` in `./Makefile`. This can take a long time, so prefer `make asterinas ASTERINAS_TEST=my-case` when checking a change. Always remember to add the new `my-case` kernel test to `ASTERINAS_TESTS`.
