---
name: normalize-local-path-for-skill-contents
description: Keep local filesystem paths in KMiri project skills portable by expressing them relative to the KMiri repository root, without machine-specific absolute paths.
---

# Normalize local paths in project skills

Use this convention whenever you write or update a skill under `./.agents/skills/` in the KMiri repository. All project skills should follow it.

## Path base

Treat the KMiri repository root (the directory containing `./miri`, `./Makefile`, and `./tests`) as the base for paths in skill prose and command examples. Prefix in-repository paths with `./` and sibling workspace paths with `../`, for example:

- `./tests/pass/kmiri/example.rs`
- `./dev/test_utils/example.rs`
- `../tests/my-case/src/lib.rs`
- `../asterinas/ostd`

Do not put machine-specific absolute paths, home directories, or paths from a particular user's checkout in project skills. When a command must run from a subdirectory, state that working directory and express the `cd` from the repository root, or use a path valid from that stated directory.

## Preserve tool-specific path bases

Relative paths are interpreted by the tool that consumes them. Keep examples valid for their actual base and state that base when it is not obvious:

- Shell paths are relative to the command's current working directory.
- Cargo dependency paths are relative to the `Cargo.toml` that declares them.
- Rust `#[path = "..."]` paths are relative to the source file containing the attribute. For a test in `./tests/pass/kmiri/`, referencing shared code in `./dev/` therefore uses `../../../dev/...` in the attribute.
- `//@compile-flags` values are interpreted by KMiri's test harness; follow the base used by neighboring test cases.

When changing a skill, check that every path and command still resolves from the stated base. Apply this skill's convention to all other project skills as they are created or updated.
