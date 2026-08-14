# TODO

## ShortCStr enhancements

- [x] Add `ShortCStr::split()` — iterator yielding `ShortCStr` items, splitting on a separator byte
- [x] Add `ShortCStr::parse<T: FromStr>()` — convert ShortCStr to typed values (e.g. `i32`, `u32`)
- [x] Add `push_cstr(&mut self, str: &CStr)` — infallible since `CStr` doesn't contain NUL bytes by definition
- [x] Add `NoNul` unsafe trait implemented for all types that cannot contain NUL bytes; use it to extend infallible pushes (`push`, `concat`) to such types (e.g. `CStr`, `CString`, `ShortCStr`)

## Refactoring / cleanup

- [x] `dollar.rs` — extracted, now 60 lines
- [x] `importedfd.rs` — split into `importedfd_error.rs` + `importedfd_try.rs`
- [ ] `caret.rs` at 108 code lines (was 88, grew) — extract tests to `caret/tests.rs`
- [x] `substitute/brace.rs` — extracted, now 65 lines
- [x] `parse/token.rs` — combined whitespace/separators, separated `$`/`` ` `` arms, removed dead token_emit.rs (93 → 78 lines)
- [ ] `intercept/read/io.rs` at 85 code lines — extract `SourceFd::RawFd` read loop
- [ ] `exec/mod.rs` at 84 code lines — `exec_fd`/`exec_at` share duplicated setup; extract
- [ ] `openat2/parse/mod.rs` at 90 code lines
- [ ] `localfd.rs` at 114 code lines (was 80, grew) — extract `read_all` to reduce below 80
- [ ] Add `exec_fd`/`exec_at` to `safe/builtins/` crate (parse modules + integration tests)
- [ ] Drop `no_std` on `unsafe/sys` — replace `IoVec`/`IoVecMut` with `std::io::IoSlice`/`IoSliceMut`
- [ ] `FdPassError::SendFailed` in `child/fdpass.rs:23` used for both `try_into_local()` (CLOEXEC) and `send_fd()` (socket send) — split into `FdPassError::Cloexec` so error variants are not too coarse per LESSONS.md
- [ ] `environ.rs` at 51 code lines with 4 levels of nesting in `exports_iter` closure (§2.4 limit) — extract filter + concat logic into a helper function
- [ ] `comment.rs::scan_block` has 5 levels of logical nesting (while → if is_comment||is_sep → for sub → if !sub.is_empty() → match) exceeding §2.4 limit of 4 — extract keyword delta processing into a helper

## Bash compatibility gaps

### P0 — Easy wins

- [ ] `$_` — last argument of previous command
- [ ] `$-` — shell option flags
- [ ] `type` builtin — show command type (builtin, external, fd var, etc.)
- [ ] `command` builtin — bypass function lookup (alias for `builtin` prefix)

### P1 — Major functionality gaps (moderate effort)

- [ ] `test` / `[` builtin — file tests (`-f`, `-d`, `-e`), string tests (`=`, `!=`, `-z`, `-n`), numeric tests (`-eq`, `-lt`, `-gt`)
- [ ] `printf` builtin — format string output
- [ ] `set --` — replace positional parameters
- [ ] `${var:-default}`, `${var:=default}`, `${var:+alt}`, `${var:?err}` — parameter expansion operators
- [ ] `${!indirect}` — indirect variable expansion
- [ ] `exec` builtin — redirect-only mode (`exec N>&file`)
- [ ] `eval` — parse and execute constructed string
- [ ] `source` / `.` — execute script file in current shell
- [x] `break` / `continue` — loop control (for/while/until)
- [ ] Here-strings (`<<<"string"`) — pipe string into command's stdin
- [ ] `>&` / `<&` fd dup redirects — `echo hello 2>&1`, `exec 5>&1`
- [ ] `<>` — open file for read/write
- [ ] `/dev/fd/N` — automatic fd path translation
- [ ] Word splitting after unquoted `$` expansion — split on IFS when assignment is unquoted
- [ ] `shopt` / `set -o` — shell options bitmask
- [ ] Alias expansion — text-replacement pass on command words
- [ ] `hash` — PATH lookup cache
- [ ] `ulimit` — resource limit get/set
- [ ] `return` builtin (requires functions)

### P2 — Important for bash compatibility (hard)

- [ ] Heredocs (`<<EOF`) — multi-line stdin from script body with delimiter scanning
- [ ] Glob expansion (`*`, `?`, `[...]`) — expand patterns to matching filenames
- [ ] Arithmetic expansion `$((expr))` — integer expression evaluation
- [ ] Brace expansion (`{a,b,c}`, `{1..5}`) — pre-tokenization string generation
- [ ] Functions — `name() { body; }` with scoped locals, call stack, `return`
- [x] `case` / `esac` — pattern matching with `;;` separators
- [ ] Job control — `bg`, `fg`, `jobs`, SIGTSTP handling, TTY pgrp management
- [ ] Process substitution (`<(cmd)`, `>(cmd)`) — fifo/pipe with /dev/fd path
- [ ] `"$@"` preservation — expand to multiple words preserving empty args
- [ ] History expansion (`!!`, `!echo`) — readline-style history

## Tests

- [x] Fix parallel test interference — `test_captures_success` and `resolve_path_finds_dot_slash` fail when run with other tests in parallel but pass individually; run with `--test-threads=1` or identify shared state / file system collisions
- [x] Add unit test for `get_environ` — currently only covered via fork+exec integration; needs isolated test verifying output vector contents (FDSHELL_PID present, filtered vars excluded, exports merged)

## Security / hardening

### P0 — Script-reachable crash/hang (fix first)

- [x] `export_to + 1` overflow — `2147483647>%var` redirect panics in debug (`attempt to add with overflow`) and wraps to `i32::MIN` → EINVAL in release (`redirect/resolve.rs:22`); fixed with `checked_add` → `OpenRedirectError::FdNumberOutOfRange` (+ regression test)
- [ ] Deeply nested `if` — O(n²) parse (re-parses remaining body per level, `if_exec.rs:9`) + unbounded in-process recursion → 23s at n=4000 (release), stack-overflow SIGABRT in debug; tokenize once per line, cap nesting depth

### P1 — DoS / hardening

- [ ] `cmd_subst::run_and_capture` accumulates output with no cap — `echo $(yes)` → unbounded memory (`cmd_subst.rs`); apply size limit
- [ ] `recv_fd` pid verification is best-effort — SCM_CREDENTIALS checked only if delivered; make mandatory (`shellfd/recv_fd.rs`)
- [ ] `FDSHELL_PID`/`FDSHELL_SOCKET` trust — wrapper can spoof nested-shell env and capture exported fds (`init.rs`); document/limit trust boundary

### P2 — Hardening / informational

- [ ] Numbered path redirects at `i32::MAX` (`2147483647>file`) fail at `dup2` with generic "failed to open redirection path" — apply the same range check as the var branch for a consistent "file descriptor number is out of range" (`redirect/mod.rs:29`)
- [ ] Non-CLOEXEC socket fd leaks into nested-shell grandchildren via `FDSHELL_SOCKET` — ensure CLOEXEC or strip in children
- [ ] `ExportedCStr::as_ref` uses `unreachable_unchecked`; tail-`Static` `as_cstr_bytes` ignores `length` — sound under current invariants but UB-fragile; add safety comment/invariant test (`shortcstr/access.rs`)

## Open Directions

### P0 — Protocol spec + external integration

- [ ] Specify and version the FDSHELL_SOCKET protocol (message format, tags, error reporting, feature negotiation via env var)
- [ ] Ship `fdsend` helper binary so any program can return fds unmodified
- [ ] Client libraries for C/Rust/Python/Go (~50 lines each)
- [ ] Readiness signaling convention (tag-only "ready" message as race-free sd_notify alternative)

### P1 — Core syscall builtins

- [ ] `timerfd` syscall wrapper + builtin; `wait --any` + `--timeout` via timerfd polling
- [ ] `signalfd` builtin — traps as another fd source
- [ ] `eventfd` builtin — counters between background tasks
- [ ] Landlock syscall wrappers + builtin (`landlock --allow-rw %src --restrict`)
- [ ] `pidfd_send_signal` builtin — kill background jobs by pidfd var
- [ ] fs-verity ioctls (verify binary before execveat)

### P1 — Language features

- [ ] Lexical scoping / RAII for fd vars: auto-close at block end, linear-use check (use-after-unset + leaks as parse-time errors)
- [ ] Structured return channel: extend socket protocol to carry payloads (statx results, readlink targets, error strings) alongside fds

### P2 — Syscall coverage

- [ ] `splice`/`copy_file_range`/`sendfile` builtins → zero-copy cat/cp
- [ ] `memfd_create` builtin — heredocs without temp files, sealed secrets by fd
- [ ] `O_TMPFILE` + `linkat` atomic file creation pattern
- [ ] `FICLONE` ioctl for reflinks
- [ ] More `*at` coverage: `symlinkat`, `linkat`, `statx`, `utimensat` (unlinkat syscall wrapper exists, no builtin)
- [ ] `flock`, `ftruncate`, `lseek`, `fsync` on existing fd vars
- [ ] `getdents64` for directory listing

### P2 — Language features

- [ ] Typed fd vars: dir vs file vs pipe-end vs socket vs pidfd, checked against builtin expectations
- [ ] Coprocesses and process substitution: `coproc name { cmd }` with bidirectional pipe fd vars
- [ ] Escape hatch expansion: `%var:path` → `/proc/self/fd/63` for programs that only accept paths
- [ ] `poll`/`epoll` builtin → event-driven scripts (supervisors, watchers)

### P3 — Security directions

- [ ] Strict mode: ban absolute path resolution entirely; every operation relative to an explicit dirfd — capability shell in Capsicum spirit
- [ ] Broker pattern: make socket bidirectional so sandboxed child can request an open; privileged shell resolves against its dirfds
- [ ] Provenance/audit: tag every fd with origin; `fdexplain %foo` → "opened by openat2 from %CWD, line 3"
- [ ] Leak-detector test mode: snapshot `/proc/self/fd` before/after script run, assert no stragglers; verify CLOEXEC invariants

### P3 — Application domains (emerge from above)

- [ ] Init/supervision: pidfds + readiness tags + `wait --any` + restart policies
- [ ] Mini container runtime: namespaces + new mount API + landlock + seccomp, orchestrated in script
- [ ] Busybox-style multicall binary: builtins symlinked as cat, mv, ls via getdents64 on dirfd

### P3 — Engineering / ecosystem

- [ ] aarch64/riscv64 ports (sys crate already isolates syscall numbers); static musl builds
- [ ] Kernel feature detection with documented degradation (openat2 needs 5.6+, pidfds 5.3+)
- [ ] Parser fuzzing
- [ ] ShellCheck-style linter (fd leaks, unset-in-branch, missing wait)
- [ ] "Writing TOCTOU-free scripts" guide
