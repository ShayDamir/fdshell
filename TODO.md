# TODO

## ShortCStr enhancements

- [x] Add `ShortCStr::split()` — iterator yielding `ShortCStr` items, splitting on a separator byte
- [x] Add `ShortCStr::parse<T: FromStr>()` — convert ShortCStr to typed values (e.g. `i32`, `u32`)
- [x] Add `push_cstr(&mut self, str: &CStr)` — infallible since `CStr` doesn't contain NUL bytes by definition
- [x] Add `NoNul` unsafe trait implemented for all types that cannot contain NUL bytes; use it to extend infallible pushes (`push`, `concat`) to such types (e.g. `CStr`, `CString`, `ShortCStr`)

## Refactoring / cleanup

- [x] `dollar.rs` — extracted, now 60 lines
- [x] `importedfd.rs` — split into `importedfd_error.rs` + `importedfd_try.rs`
- [x] `caret.rs` — extracted tests to `caret/tests.rs`, now 21 code lines
- [x] `substitute/brace.rs` — extracted, now 65 lines
- [x] `parse/token.rs` — combined whitespace/separators, separated `$`/`` ` `` arms, removed dead token_emit.rs (93 → 78 lines)
- [x] `intercept/read/io.rs` at 85 code lines — extract `SourceFd::RawFd` read loop
- [x] `exec/mod.rs` — extracted duplicated `exec_fd`/`exec_at` setup (sock export + `get_environ`) into `prepare_envp`, moved PATH lookup to `search.rs` (74 → 62 code lines)
- [x] `openat2/parse/mod.rs` — extracted arg loop to `openat2/parse/args.rs` (90 → 36 code lines)
- [x] `localfd.rs` at 114 code lines (was 80, grew) — extracted `read_all` loop to `rw.rs` free fn, method now delegates (74 → 61 code lines)
- [x] Add `exec_fd`/`exec_at` to `safe/builtins/` crate — `execfd`/`execat` modules with parse + exec, `builtin_exec_ok` test helper, integration tests; fdshell `exec`/handlers now delegate to them
- [x] `FdPassError::SendFailed` in `child/fdpass.rs:23` used for both `try_into_local()` (CLOEXEC) and `send_fd()` (socket send) — split into `FdPassError::Cloexec` so error variants are not too coarse per LESSONS.md
- [x] `environ.rs` at 51 code lines with 4 levels of nesting in `exports_iter` closure (§2.4 limit) — already extracted to `export_entry` helper (commit a9a8abf); now 41 code lines, max depth 3
- [x] `comment.rs::scan_block` has 5 levels of logical nesting (while → if is_comment||is_sep → for sub → if !sub.is_empty() → match) exceeding §2.4 limit of 4 — extracted to `depth_delta` helper; also fixed a script-reachable u32 underflow panic on unbalanced closers (`if true; then fi fi; fi` now fails gracefully instead of panicking)
- [ ] `parse/case_clause.rs` at 86 code lines (80-90 zone) — extract the body-slice extraction in `parse_clauses` into a helper
- [ ] `shellfd/recv_fd.rs` has 5 levels of logical nesting (while → if tuple → if let split_first → for rest) exceeding §2.4 limit of 4 — extract the SCM_RIGHTS handler into a helper

## Bash compatibility gaps

### P0 — Easy wins

- [ ] `$_` — last argument of previous command
- [ ] `$-` — shell option flags
- [ ] `type` builtin — show command type (builtin, external, fd var, etc.)
- [ ] `command` builtin — bypass function lookup (alias for `builtin` prefix)
- [ ] Allow environment variables to become shell variables — `FOO=bar fdshell -c 'builtin echo $FOO'` should output `bar` (resolve `$FOO` against the inherited environ when not set in `state.strings`)

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
- [x] Deeply nested `if`/`while`/`until`/`for`/`case` — superlinear O(n²) CPU: each nesting level re-runs `scan_block`'s keyword scan + `try_join` over the remaining body (`segment.rs:62`, `script.rs:17-30` re-tokenizes the body). Verified: single-line `if true; then … ;fi` chain → 100→25ms, 500→160ms, 2000→2.2s, 4000→13s (release); 20000 → >120s (both release and debug, CPU-bound; no stack overflow because nesting is shallow — `parse` stores bodies as strings and `run_if`/`run_loop`/`run_for` recurse only ~n frames at runtime). Fixed by capping execution nesting at `nest::MAX_NESTING` (100) via a `ShellState::nesting` counter incremented in `nest::deeper` at every block-body and `$(…)` entry point; the counter is inherited by forked `$(…)` children so command-substitution nesting is bounded too; worst case is now O(MAX_NESTING × script size): the 4000-level chain goes from 13s to ~180ms and fails with a clean `NestingTooDeep` error (+ unit + integration tests in `tests/nest.rs`)

### P1 — DoS / hardening

- [x] `cmd_subst::run_and_capture` accumulates output with no cap — `echo $(yes)` → unbounded memory (`cmd_subst.rs`); apply size limit. Fixed: `drain` is capped at `MAX_CAPTURED` (64 MiB) via a new `CmdSubstError::OutputTooLarge`; on overflow the read end is dropped and the child is killed via a new `pidfd_send_signal` wrapper so an unbounded producer can't linger or hang the shell
- [ ] `set --stdout-capture-limit <bytes>` — make the `$(…)` stdout capture cap configurable; `MAX_CAPTURED` is hardcoded at 64 MiB (`cmd_subst.rs:13`); bash has no such limit, so this is an fdshell-specific escape hatch for scripts that legitimately capture more than the default
- [ ] `recv_fd` pid verification is best-effort — SCM_CREDENTIALS checked only if delivered; make mandatory (`shellfd/recv_fd.rs`)
- [ ] `FDSHELL_PID`/`FDSHELL_SOCKET` trust — wrapper can spoof nested-shell env and capture exported fds (`init.rs`); document/limit trust boundary
- [ ] `~` / `$HOME` escape the capability model — the shell operates on fd-vars (`%CWD`) but `~` expansion (`substitute/arg.rs:24`) and `cd_home` (`cd/mod.rs:20`) open the inherited `$HOME` via *absolute* path with default `openat2` flags (no `RESOLVE_BENEATH`, no `O_NOFOLLOW`); a symlink at `$HOME` (or inside it) silently redirects file ops / `cd` to an attacker-controlled location, and `~` reaches outside any `RESOLVE_BENEATH` sandbox. Resolve `~` against a controlled dirfd, or drop `~` in strict mode

### P2 — Hardening / informational

- [ ] Numbered path redirects at `i32::MAX` (`true 2147483647>file`) fail at `dup2` with a generic "failed to open redirection path" / `EBADF` (verified: `true 2147483647>%f` gives the clean "file descriptor number is out of range" but the path branch does not) — apply the same range check as the var branch (`redirect/resolve.rs:24-25` is var-only; add it for `RedirectSource::Path` in `resolve_redirects` or range-check `export_to` at parse time in `parse/redirect.rs:34`)
- [ ] Non-CLOEXEC socket fd leaks into nested-shell grandchildren via `FDSHELL_SOCKET` — ensure CLOEXEC or strip in children
- [ ] `ExportedCStr::as_ref` uses `unreachable_unchecked`; tail-`Static` `as_cstr_bytes` ignores `length` — sound under current invariants but UB-fragile; add safety comment/invariant test (`shortcstr/access.rs`)
- [ ] Unbounded script size — `cli::load_script` and the `-c`/stdin paths read the entire script into a `Vec<u8>` with no cap (`cli.rs:7`); a multi-GB script / `-c` argument OOMs the shell before parsing (compounds with the nested-`if` O(n²) CPU). Add a max-script-size check
- [ ] `getcwd` fixed 4096-byte buffer — `env::getcwd` (`env.rs:37`) fails with `ENAMETOOLONG` (surfaced as a generic `BuiltinError::Io`) when the CWD path exceeds 4096 bytes; `pwd` then gives an unactionable error. Read the cwd via `/proc/self/cwd` (readlink) or grow the buffer so `pwd` keeps working for deep directory trees

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
- [ ] Provenance/audit: tag every fd with origin; `fdexplain %foo` → "opened by openat2 from %CWD, line 3" — design is reusable from string provenance: `Origin`/`Trace`/`ScriptText` in `unsafe/sys/src/importedstr/` already track `(Position, Origin)` for strings and the `explain` builtin renders them; attach a `Trace` to fd vars at acquisition points and add an `fdexplain` builtin
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
