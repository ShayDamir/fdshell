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

Three fd types across `unsafe/sys/src/`:

| Type | Owns? | CLOEXEC? | Drop closes? | `const fn from_raw` | `from_bytes` (validated) | Module |
|---|---|---|---|---|---|---|
| `Fd` | yes | yes | yes | `unsafe` | n/a | `fd.rs` |
| `DupFd` | no | no | no | `unsafe` | safe (`verify()`) | `dupfd.rs` |
| `AtFd<'a>` | no | irrelevant | no | `unsafe` | n/a (via `From<&Fd>` / `From<&DupFd>`) | `atfd.rs` |

- `Fd::verify()` and `DupFd::verify()` return `Result<(), i32>` using `cvt` (never `__errno_location`).
  `Fd::verify` checks CLOEXEC is SET; `DupFd::verify` checks CLOEXEC is CLEAR.
- `DupFd::from_bytes` delegates to `verify()` — validates fd is open AND non-CLOEXEC.
- `DupFd::from_raw` is `unsafe` — only for trusted constants (`SHELLFD`) and kernel returns (`dup`/`dup2`).
  Direct construction `DupFd(n)` is **not** allowed — always use `unsafe { DupFd::from_raw(n) }` with `// SAFETY:`.
- `AT_FDCWD` stays entirely in `atfd.rs` (`AtFd::cwd()`). Never re-exported.
- `AtFd` is `Copy + Clone` — used multiple times in exec functions.
- `*at` syscall wrappers take `AtFd<'_>` or `Option<AtFd<'_>>` instead of raw `i32`.

## `unsafe/sys/src/` module layout

| Module | Role |
|---|---|
| `atfd.rs` | `AtFd<'a>` — non-owning borrowed fd for `*at` syscalls |
| `dupfd.rs` | `DupFd` — non-owned fd for child-process inheritance |
| `fd.rs` | `Fd` — owned fd with Drop |
| `rw.rs` | fd I/O — `read`, `write` |
| `fcntl.rs` | Re-exports O\_\* and fcntl constants from `libc` |
| `mkdirat.rs` | Directory creation — `mkdirat(dirfd, path, mode)` |
| `renameat2.rs` | Rename — `renameat2(olddirfd, oldpath, newdirfd, newpath, flags)` + `RENAME_*` constants |
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

## Launch / Capture

- `launch()` in `safe/fdshell/src/launch.rs` is stateless — no capture logic, no `&mut FdVars`.
  Returns `Result<(WaitStatus, Fd), i32>` — the `Fd` is the parent end of the capture socket.
- `do_captures()` in `safe/fdshell/src/capture.rs` owns both the capture socket and captures vec
  (takes `captures: Vec<Capture>` by value). Returns `Vec<(CString, Fd)>` on success.
  The caller commits atomically into `fdvars` after a successful receive-and-stage phase.
  Captures are received only when `status == Exited(0)` (status gate).
- `Capture { var: CString, tag: Option<CString>, force: bool }`:
  - `force = false` → `New` (`%>%var`): fail `EEXIST` if var already exists.
  - `force = true` → `Override` (`%>|%var`): existing fd dropped via `HashMap::insert`.
  - `tag = None` → positional; `tag = Some("rd")` → tagged match (`%rd>%var`).
- Matching is receiver-driven: `do_captures` loops until captures are exhausted. For each received
  `(fd, tag)`, it scans captures: tagged match first, then positional fallback. Unknown fds
  (no matching capture) are silently closed.
- Parser must guarantee unique target variables in captures — `do_captures` checks against
  committed `fdvars` state only (no scan of staged `captured_fds` vec).
- `Redirect { target_fd: i32, src_var: CString }` — `target_fd` is the fd number to `dup_to`
  onto (0 for `<`, 1 for `>`, 2 for `2>`), `src_var` is the `%var` holding the source fd.
  Applied in the child after `SHELLFD` dup_to but before builtin dispatch.
- `Fd::dup_to(new: i32)` returns `DupFd` — kernel always returns `new` on success.
  Takes a raw fd number, not a `DupFd`. Used for both SHELLFD reservation and redirects.
- `Fd::try_clone(new: i32)` returns `Fd` — owned CLOEXEC copy at `new` (wraps `dup3`).
  Used for `%var=%var2` syntax to produce a new CLOEXEC fd.
- `substitute_arg() in `safe/fdshell/src/resolve.rs` resolves `%var` references in argument
  strings.  Each `%var` calls `fd.dup()` once per distinct variable via a
  `HashMap<CString, DupFd>` cache — repeated `%foo` in the same command line
  produces the same fd number, preserving fd equality for subprocesses.
