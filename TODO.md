# TODO

## ShortCStr enhancements

- [x] Add `ShortCStr::split()` — iterator yielding `ShortCStr` items, splitting on a separator byte
- [x] Add `ShortCStr::parse<T: FromStr>()` — convert ShortCStr to typed values (e.g. `i32`, `u32`)

## Refactoring / cleanup

- [ ] `dollar.rs` at 104 code lines (14 over 90-line limit) — extract `$@`, `$*`, `$N` handlers into helpers
- [ ] `importedfd.rs` (unsafe/sys) at 106 code lines (16 over) — split test module or extract verification logic
- [ ] `caret.rs` at 88 code lines (in 80-90 flag zone) — tests dominate; extract to `caret/tests.rs`
- [ ] `substitute/brace.rs` at 87 code lines (in 80-90 flag zone) — extract closed vs unclosed brace handling
- [ ] `parse/token.rs` at 87 code lines (in 80-90 flag zone) — consider extracting `tokenize` match arms into separate helpers
- [ ] `intercept/read/io.rs` at 87 code lines (in 80-90 flag zone) — extract `SourceFd::RawFd` read loop
- [ ] `exec/mod.rs` at 82 code lines (in 80-90 flag zone) — `exec_fd`/`exec_at` share duplicated setup; extract
- [ ] `openat2/parse/mod.rs` at 81 code lines (in 80-90 flag zone)
- [ ] `localfd.rs` at 80 code lines (in 80-90 flag zone) — extract `read_all` to reduce below 80
- [ ] Add `exec_fd`/`exec_at` to `safe/builtins/` crate (parse modules + integration tests)
- [ ] Drop `no_std` on `unsafe/sys` — replace `IoVec`/`IoVecMut` with `std::io::IoSlice`/`IoSliceMut`
- [ ] `FdPassError::SendFailed` in `child/fdpass.rs:23` used for both `try_into_local()` (CLOEXEC) and `send_fd()` (socket send) — split into `FdPassError::Cloexec` so error variants are not too coarse per LESSONS.md
- [ ] `environ.rs` at 51 code lines with 4 levels of nesting in `exports_iter` closure (§2.4 limit) — extract filter + concat logic into a helper function

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

- [ ] Fix parallel test interference — `test_captures_success` and `resolve_path_finds_dot_slash` fail when run with other tests in parallel but pass individually; run with `--test-threads=1` or identify shared state / file system collisions
- [ ] Add unit test for `get_environ` — currently only covered via fork+exec integration; needs isolated test verifying output vector contents (FDSHELL_PID present, filtered vars excluded, exports merged)

## Security / hardening

(All items complete)
