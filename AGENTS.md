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

- Tests live in `unsafe/sys/tests/` and `safe/builtins/tests/`.
- `useNextest = true` in `package.nix` / `flake.nix`.
- Run tests: `cargo test -p sys -p builtins`.

## Platform

- Linux x86_64 only (for now). Static binary target.
- Flag constants come from `sys::fcntl` (re-exported from `libc`).
  Never hardcode — values differ between architectures.

## `unsafe/sys/src/` module layout

| Module | Role |
|---|---|
| `fd.rs` | Meta-level fd manipulation — `close`, `dup2`, `dup3` |
| `rw.rs` | fd I/O — `read`, `write` |
| `fcntl.rs` | Re-exports O\_\* and fcntl constants from `libc` |
| `mkdirat.rs` | Directory creation — `mkdirat(dirfd, path, mode)` |
| `renameat.rs` | Rename — `renameat(olddirfd, oldpath, newdirfd, newpath)` |
| `openat2.rs` | `openat2` syscall, `OpenHow`, `RESOLVE_*` constants |
| `pipe.rs` | Pipe — `pipe2(flags)` |
| `shellfd.rs` | SHELLFD protocol — `send_fd`, `recv_fd` |
| `stat.rs` | `FileStat`, `stat`, `fstat` |

## Builtin conventions

- **SHELLFD tags** are per-builtin constants (`c"openat2"`, `c"dirfd"`),
  one per builtin, never depend on arguments.
- **Always produce fds with `O_CLOEXEC`** — every exec function ORs
  `O_CLOEXEC` into the flags. Only strip it via `dup` if the caller
  explicitly wants a non-CLOEXEC fd.
- **No hardcoded flag constants** — use `sys::fcntl`. Values differ
  between architectures.
- **`mkdirat` race**: the kernel has no atomic create-directory-and-
  return-fd operation. `mkdirat_exec` does `mkdirat` → `openat2`. The
  race is accepted for a shell context.
