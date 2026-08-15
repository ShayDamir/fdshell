# FD Shell — agent guidance

Read [`STYLE.md`] and [`LESSONS.md`] before changing code; add new lessons there.

## Workspace
`resolver = "2"`; three `#![no_std]` crates: `safe/fdshell/` (bin, `forbid(unsafe_code)`, shell logic), `safe/builtins/` (lib, `forbid(unsafe_code)`, builtins), `unsafe/sys/` (lib, unsafe, syscalls — the only crate with raw fds). Safe crates never call libc. Syscall wrappers return `Result<_, SyscallError>` via `cvt()`. Platform: Linux x86_64 only.

## Lints
Deny: `clippy::unwrap_used`, `expect_used`, `indexing_slicing`, `undocumented_unsafe_blocks`. Allow: `dead_code`, `clippy::todo`.

## Commands
`cargo build`; `cargo fmt`; `cargo clippy -- -D warnings`; `nix build` (→ `result/bin/fdshell`); `nix flake check --build-all` (fmt + clippy + nextest). Version from `safe/fdshell/Cargo.toml`; `git add` nix files first. `package.nix` params: `doFmt`, `doClippy`, `doTests`, `doCoverage`.

## Execution pipeline (`safe/fdshell/src/`)
`script.rs` = `run_script` (split on `;`/`\n`), `cond.rs` = `run_cond_list` (split `&&`/`||`), `run.rs` = `run_one` (parse + dispatch). `if`/`fi`: split on space mid-segment to catch keywords; unmatched `if` → `EINVAL`. Separators apply only outside quotes.

## Testing
`cargo nextest run --status-level fail --show-progress none`; integration tests in `unsafe/sys/tests/` and `safe/builtins/tests/`; unit tests in separate `<module>/tests.rs` files (inline `mod tests {}` forbidden — STYLE.md §2.8). **Never `cargo test`** — its shared harness breaks `fork()`-based tests (hangs, fd corruption, interference).

## Coverage
`nix build .#coverage` (after `git add`) → `result/index.html` + `result/coverage-report.txt`.

## FD types
Spec: [`STYLE.md`] §5. No raw fds outside `unsafe/sys`.

## Builtins
SHELLFD tags are per-builtin constants (`c"openat2"`, `c"dirfd"`). Always `O_CLOEXEC` (strip via `dup` if needed). No hardcoded constants: `libc::` in sys, re-exported in safe crates. `mkdirat` race accepted.

## Errors
Spec: [`STYLE.md`] §4. Clean, concise, actionable. Cross-crate: `.change_context()`; add a variant if none fits; preserve the error chain.
