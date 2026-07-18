# FD Shell — agent guidance

## Lessons

All plans/implementations must align to [`LESSONS.md`] and [`STYLE.md`]. Check before implementing; add new issues as lessons.

## Workspace

Three crates (`resolver = "2"`):

| Path | Type | Key attributes | Role |
|---|---|---|---|
| `safe/fdshell/` | bin | `#![no_std]`, `#![forbid(unsafe_code)]` | Shell logic, fd passing |
| `safe/builtins/` | lib | `#![no_std]`, `#![forbid(unsafe_code)]` | Builtin commands |
| `unsafe/sys/` | lib | `#![no_std]`, unsafe allowed | Syscall wrappers |

- `safe/` crates cannot call libc directly.
- File length: see [`STYLE.md`] §2.
- Unsafe conventions: [`STYLE.md`] §7.
- Safe syscall wrappers return `Result<_, SyscallError>` via `cvt()`.
- All crates are `#![no_std]`.

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
nix flake check --build-all # fmt + clippy + nextest
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

Linux x86_64 only for now.

## FD types

Full spec in [`STYLE.md`] §5. Key rules:
- Never use raw file descriptors outside of `sys` crate

## Builtin conventions

- SHELLFD tags are per-builtin constants (`c"openat2"`, `c"dirfd"`).
- Always produce fds with `O_CLOEXEC`. Strip via `dup` if needed.
- No hardcoded constants — use named constants: in sys use `libc::` constants, re-export is needed in safe crates.
- `mkdirat` race (no atomic mkdir+open): accepted for shell context.

## Error handling

All error messages must be clean, concise, actionable. Cross-crate boundaries: `.change_context()`. Add new variant if none fits. Preserve error chain at all cost. Full spec in [`STYLE.md`] §4
