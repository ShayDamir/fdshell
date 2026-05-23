# FD Shell — agent guidance

## Workspace layout

Three crates in a Cargo workspace (`resolver = "2"`):

| Path | Type | Key attributes | Role |
|---|---|---|---|
| `safe/fdshell/` | binary | `#![forbid(unsafe_code)]`, uses `std` | Main shell logic, fd passing |
| `safe/builtins/` | lib | `#![no_std]`, `#![forbid(unsafe_code)]` | Builtin commands |
| `unsafe/sys/` | lib | `#![no_std]`, unsafe allowed | Syscall wrappers for safe crates |

- `safe/` crates **cannot** call libc directly (`forbid(unsafe_code)`).
- `unsafe/` source files must be ≤80 lines each (excluding comments).
- Every `unsafe` block **must** have a preceding `// SAFETY:` comment explaining why preconditions are met.
- Safe wrappers in `unsafe/sys` return `Result<_, i32>` (positive errno on error). Use `cvt(ret: isize) -> Result<isize, i32>` from `lib.rs` to convert libc return values.
- Avoid `#[derive]` where possible. Prefer `no_std` when feasible (binary uses `std` because `no_std` + stable needs nightly + `-Z build-std`).

## Lints (workspace-wide)

| Lint | Severity | Note |
|---|---|---|
| `dead_code` | allow | Toggle to `deny` when crate APIs stabilize |
| `clippy::todo` | allow | Toggle to `deny` when ready |
| `clippy::unwrap_used` | deny | Use `?` or pattern matching |
| `clippy::expect_used` | deny | Same as above |
| `clippy::indexing_slicing` | deny | Use `.get()` / `.get_mut()` |

## Commands

```sh
cargo build                  # native build
cargo clippy -- -D warnings  # lint (all warnings denied)
```

CI runs via Nix flake:

```sh
nix build                   # builds release binary → result/bin/fdshell
nix flake check             # clippy + cargo nextest
```

- Version is read from `safe/fdshell/Cargo.toml` in `flake.nix`.
- Nix files must be `git add`-ed before `nix build`/`nix flake check` (Nix reads from the Git index).
- `package.nix` parameters: `doClippy`, `doTests`. Future: add `doCoverage ? false` the same way.

## Testing

- Tests live in `unsafe/sys/tests/` (one integration test: `shellfd.rs`).
- `useNextest = true` in `package.nix` / `flake.nix`.
- Run tests: `cargo test -p sys`.

## Platform

- Linux x86_64 only. Static binary target (no libc dependency at runtime).
