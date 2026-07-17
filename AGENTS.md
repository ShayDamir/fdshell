# FD Shell — agent guidance

## Lessons

All plans/implementations must align to [`LESSONS.md`](../LESSONS.md). Check before implementing; add new issues as lessons.

## Workspace

Three crates (`resolver = "2"`):

| Path | Type | Key attributes | Role |
|---|---|---|---|
| `safe/fdshell/` | bin | `#![forbid(unsafe_code)]`, `std` | Shell logic, fd passing |
| `safe/builtins/` | lib | `#![no_std]`, `#![forbid(unsafe_code)]` | Builtin commands |
| `unsafe/sys/` | lib | `#![no_std]`, unsafe allowed | Syscall wrappers |

- `safe/` crates cannot call libc directly.
- Source files ≤80 code lines (excl. tests). Measure via `cargo fmt` then `tokei`. Don't compress formatting.
- Every `unsafe` block needs preceding `// SAFETY:` comment.
- Safe syscall wrappers return `Result<_, SyscallError>` via `cvt()`.
- Avoid `#[derive]` in production — quarantine with `#[cfg_attr(test, derive(...))]`.
  Exceptions: `Display`+`Debug` on error enums (`core::error::Error`), `Debug` on error-stack attachment types, `ShortCStr`'s `Debug`/`PartialEq`/`Eq`.
- Prefer `no_std` where feasible.

## Lints

| Lint | Severity |
|---|---|
| `dead_code` | allow |
| `clippy::todo` | allow |
| `clippy::unwrap_used` | deny |
| `clippy::expect_used` | deny |
| `clippy::indexing_slicing` | deny |

## Commands

```sh
cargo build            # native build
cargo fmt              # format
cargo clippy -- -D warnings
nix build              # release → result/bin/fdshell
nix flake check        # fmt + clippy + nextest
```

- Version from `safe/fdshell/Cargo.toml`. Nix files must be `git add`-ed first.
- `package.nix` params: `doFmt`, `doClippy`, `doTests`, `doCoverage`.

## Execution pipeline (`safe/fdshell/src/`)

| Layer | File | Role |
|---|---|---|
| `run_script` | `script.rs` | Split on `;`/`\n`, depth-track `if`/`fi` |
| `run_cond_list` | `cond.rs` | Split on `&&`/`||` |
| `run_one` | `run.rs` | Parse single statement, dispatch by type |

- `if`/`fi`: when segment starts with `if`, enter depth-tracking inner loop. Split on space mid-segment to catch keywords. `is_if_or_fi` returns `Some(true/false)` for `if`/`fi`, `None` otherwise. Unmatched `if` → `EINVAL`.
- `tokens_to_if` uses `find_preceded_by_semi` for `then`/`elif`/`else`/`fi`. `trim_semi` cleans slices, `try_join` reassembles.
- `b';'` and `b'\n'` outside quotes are statement separators.

## Testing

```sh
cargo nextest run --status-level fail --show-progress none
```

Tests in `unsafe/sys/tests/` and `safe/builtins/tests/`.

**Always use `cargo nextest`, never `cargo test`.** Regular `cargo test` runs tests in-process with a shared harness that breaks when tests call `fork()` — child processes inherit the test runner's state, causing hangs, fd corruption, and cross-test interference. `nextest` runs each test in its own isolated process, which is required for correct `fork()` semantics.

## Coverage

```sh
# git add first, then:
nix build .#coverage   # → result/index.html + result/coverage-summary.json
```

## Platform

Linux x86_64 only, static binary. Flag constants from `sys::fcntl` (never hardcode).

## FD types (`unsafe/sys/src/`)

| Type | Owns? | CLOEXEC? | Drop closes? | `from_raw` | `from_bytes` | Module |
|---|---|---|---|---|---|---|
| `LocalFd` | yes | yes | yes | `unsafe` | n/a | `localfd.rs` |
| `ImportedFd` | no | no | no | `unsafe` | safe (`verify()`) | `importedfd.rs` |
| `ExportedFd` | no | no | no | `unsafe` | n/a | `exportedfd.rs` |
| `AtFd<'a>` | no | — | no | `unsafe` | n/a (via `From`) | `atfd.rs` |

- `LocalFd::verify` checks CLOEXEC SET; `ImportedFd::verify` checks CLOEXEC CLEAR.
- `ImportedFd::from_bytes` delegates to `verify()` (open + non-CLOEXEC).
- `from_raw` is `unsafe` — only for trusted constants/kernel returns.
- `AT_FDCWD` stays in `atfd.rs` (`AtFd::cwd()`), never re-exported.
- `*at` wrappers take `AtFd<'_>` or `Option<AtFd<'_>>`.
- `ImportedFd::try_into_local()` sets CLOEXEC via `fcntl`, returns `LocalFd`.

## Module layout (`unsafe/sys/src/`)

`lib.rs` (cvt, RefCStr, re-exports), `atfd.rs`, `localfd.rs`, `importedfd.rs`, `exportedfd.rs`, `rw.rs`, `fcntl.rs`, `errno.rs`, `execveat.rs`, `fchdir.rs`, `fork_pidfd.rs`, `wait_pidfd.rs`, `iovec.rs`, `mkdirat.rs`, `renameat2.rs`, `openat2.rs`, `pipe.rs`, `net.rs`, `shellfd/`, `shortcstr/`, `siginfo.rs`, `stat.rs`, `umask.rs`, `unlinkat.rs`.

## Builtin conventions

- SHELLFD tags are per-builtin constants (`c"openat2"`, `c"dirfd"`).
- Always produce fds with `O_CLOEXEC`. Strip via `dup` if needed.
- No hardcoded flag constants — use `sys::fcntl`.
- `mkdirat` race (no atomic mkdir+open): accepted for shell context.
- Dirfd in configs: `Option<DupFd>` → `DupFd::from_bytes` → `.map_or(AtFd::cwd(), DupFd::at)`.

## Error handling

All error messages must be clean, concise, actionable. Cross-crate boundaries: `.change_context()`. Add new variant if none fits. Preserve error chain.

## Nesting

`detect_nested()` in `init.rs`: if `FDSHELL_CAPTURE == getpid()` and fd 3 is open + non-CLOEXEC, we're a child fdshell. `init_shellfd()` picks:
- `Standalone(LocalFd)`: placeholder pipe at SHELLFD, CLOEXEC → closed on exec.
- `Nested(LocalFd)`: inherited socket → `try_into_local()` (set CLOEXEC) so it doesn't survive second `exec`.

## Launch / Capture

- `launch()` (`launch.rs`): stateless, returns `Result<LaunchOutcome, Report<LaunchError>>`.
- `do_captures()` (`capture.rs`): takes captures vec + socket, returns `Vec<(CString, LocalFd)>` on success.
  Commits atomically to `fdvars` only when `status == Exited(0)`.
- `Capture { var, tag, force }`: `force=false` → `%>%var` (fail `EEXIST` if exists); `force=true` → `%>|%var` (overwrite).
  Tagged match first, then positional fallback. Unknown fds silently closed.
- `Redirect { target_fd, src_var }`: `export_to` onto 0/1/2. Applied after SHELLFD export, before builtin dispatch.
- `LocalFd::export_to(new: i32)` → `ExportedFd`. `LocalFd::try_clone(new: i32)` → `LocalFd` (CLOEXEC `dup3`).
- `substitute_arg()` (`resolve.rs`): resolves `%var` via `HashMap<CString, ExportedFd>` cache (same fd for repeated `%var`).
