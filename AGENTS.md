# FD Shell — agent guidance

## Workspace layout

Three crates in a Cargo workspace (`resolver = "2"`):

| Path | Type | Key attributes | Role |
|---|---|---|---|
| `safe/fdshell/` | binary | `#![forbid(unsafe_code)]`, uses `std` | Main shell logic, fd passing |
| `safe/builtins/` | lib | `#![no_std]`, `#![forbid(unsafe_code)]` | Builtin commands |
| `unsafe/sys/` | lib | `#![no_std]`, unsafe allowed | Syscall wrappers for safe crates |

- `safe/` crates **cannot** call libc directly (`forbid(unsafe_code)`).
- `safe/` and `unsafe/` source files must be ≤80 lines each (excluding comments). If a file approaches this limit, split or refactor rather than compress formatting. Tests are exempt from the line limit.
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
cargo fmt                    # format all source
cargo clippy -- -D warnings  # lint (all warnings denied)
```

CI runs via Nix flake:

```sh
nix build                   # builds release binary → result/bin/fdshell
nix flake check             # fmt + clippy + cargo nextest
```

- Version is read from `safe/fdshell/Cargo.toml` in `flake.nix`.
- Nix files must be `git add`-ed before `nix build`/`nix flake check` (Nix reads from the Git index).
- `package.nix` parameters: `doFmt`, `doClippy`, `doTests`. Future: add `doCoverage ? false` the same way.

## Testing

- Tests live in `unsafe/sys/tests/` and `safe/builtins/tests/`.
- `useNextest = true` in `package.nix` / `flake.nix`.
- Run tests: `cargo test -p sys -p builtins`.

## Platform

- Linux x86_64 only (for now). Static binary target.
- Flag constants come from `sys::fcntl` (re-exported from `libc`).
  Never hardcode — values differ between architectures.

## FD types

Three fd types in `unsafe/sys/src/fd.rs` and `atfd.rs`:

| Type | Owns? | CLOEXEC? | Drop closes? | `const fn from_raw` | `from_bytes` (validated) |
|---|---|---|---|---|---|
| `Fd` | yes | yes | yes | `unsafe` | n/a |
| `DupFd` | no | no | no | `unsafe` | safe (`fcntl(F_GETFD)`) |
| `AtFd<'a>` | no | irrelevant | no | `unsafe` | n/a (via `From<&Fd>` / `From<&DupFd>`) |

- `Fd::at(&self) -> AtFd<'_>` — infallible (borrow + `Fd` invariant).
- `DupFd::at(&self) -> AtFd<'_>` — infallible (`from_bytes` validated at the string boundary).
- `DupFd::from_bytes` validates via `fcntl(F_GETFD)` — the only way safe code can obtain a `DupFd` from external input.
- `DupFd::from_raw` is `unsafe` — only for trusted constants (`SHELL_DUPFD`) and kernel returns (`dup`/`dup2`).
- `AT_FDCWD` stays entirely in `atfd.rs` (`AtFd::cwd()`). Never re-exported.
- `AtFd` is `Copy + Clone` — used multiple times in exec functions.
- `*at` syscall wrappers take `AtFd<'_>` or `Option<AtFd<'_>>` instead of raw `i32`.

## `unsafe/sys/src/` module layout

| Module | Role |
|---|---|
| `atfd.rs` | `AtFd<'a>` — non-owning borrowed fd for `*at` syscalls |
| `fd.rs` | `Fd` and `DupFd` types — owned fd with Drop, and non-owned fd |
| `rw.rs` | fd I/O — `read`, `write` |
| `fcntl.rs` | Re-exports O\_\* and fcntl constants from `libc` |
| `mkdirat.rs` | Directory creation — `mkdirat(dirfd, path, mode)` |
| `renameat.rs` | Rename — `renameat(olddirfd, oldpath, newdirfd, newpath)` |
| `openat2.rs` | `openat2` syscall, `OpenHow`, `RESOLVE_*` constants |
| `pipe.rs` | Pipe — `pipe2(flags)` |
| `shellfd/` | SHELLFD protocol — `send_fd`, `recv_fd` |
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
- **Dirfd in configs** is `Option<DupFd>` (`None` = CWD). Parsed via
  `parse_dirfd` → `DupFd::from_bytes` (validates fd is open). Converted
  to `AtFd` at exec time via `cfg.dirfd.as_ref().map_or(AtFd::cwd(), DupFd::at)`.
