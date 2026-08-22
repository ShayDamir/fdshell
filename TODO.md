# TODO

## Bash compatibility gaps

### P0 — Easy wins

- [ ] `$_` — last argument of previous command
- [ ] `$-` — shell option flags
- [ ] `type` builtin — show command type (builtin, external, fd var, etc.)
- [ ] `command` builtin — bypass function lookup (alias for `builtin` prefix)
- [x] Allow environment variables to become shell variables — `FOO=bar fdshell -c 'builtin echo $FOO'` should output `bar` (resolve `$FOO` against the inherited environ when not set in `state.strings`)

### P1 — Major functionality gaps (moderate effort)

- [x] `test` / `[` builtin — file tests (`-f`, `-d`, `-e`), string tests (`=`, `!=`, `-z`, `-n`), numeric tests (`-eq`, `-lt`, `-gt`)
- [x] `printf` builtin — format string output
- [x] `set --` — replace positional parameters
- [x] `${var:-default}`, `${var:=default}`, `${var:+alt}`, `${var:?err}` — parameter expansion operators
- [x] `${!indirect}` — indirect variable expansion
- [x] `exec` builtin — redirect-only mode (`exec N>&file`)
- [x] `eval` — parse and execute constructed string
- [x] `source` / `.` — execute script file in current shell
- [x] Here-strings (`<<<"string"`) — pipe string into command's stdin
- [x] `>&` / `<&` fd dup redirects — `echo hello 2>&1`, `exec 5>&1`
- [x] `<>` — open file for read/write
- [x] `/dev/fd/N` — automatic fd path translation
- [x] Word splitting after unquoted `$` expansion — split on IFS when assignment is unquoted
- [ ] IFS sync only happens on `IFS=…` assignment — `read IFS`, `export IFS=x`, `${IFS=…}`, `for IFS in …` store in `strings` without updating `state.ifs` (centralize on a `set_var` helper or sync at each insert site)
- [ ] Unquoted `$@`/`$*` join positional args with literal spaces before IFS splitting — wrong with custom IFS lacking space (injected spaces survive) and with empty IFS (collapses to one field); split per-positional instead of join-then-split
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
- [ ] Job control — `bg`, `fg`, `jobs`, SIGTSTP handling, TTY pgrp management
- [ ] Process substitution (`<(cmd)`, `>(cmd)`) — fifo/pipe with /dev/fd path
- [ ] `"$@"` preservation — expand to multiple words preserving empty args
- [ ] History expansion (`!!`, `!echo`) — readline-style history

## Refactoring

- [ ] `parse/command.rs` is 83 code lines (STYLE.md §2.3 flag zone) — extract the arg/capture/redirect loop into a helper
- [ ] `redirect/resolve.rs` is 81 code lines (STYLE.md §2.3 flag zone) — extract the per-source arm construction into helpers
- [ ] `child/test/ops.rs` is 88 code lines (STYLE.md §2.3 flag zone) — extract the file-test path/fd lookup into a helper
- [ ] `parse/token.rs` is 80 code lines (STYLE.md §2.3 flag zone) — extract the per-byte match arm into a helper
- [ ] `parse/mod.rs` is 83 code lines (STYLE.md §2.3 flag zone) — extract the keyword dispatch into a helper
- [ ] `parse/redirect.rs` is 85 code lines (STYLE.md §2.3 flag zone) — extract the `>>`/`<>` operator dispatch into a helper
- [ ] `state.rs` is 84 code lines (STYLE.md §2.3 flag zone) — extract the setter cluster into a helper module

## Security / hardening

### P1 — DoS / hardening

- [ ] `set --stdout-capture-limit <bytes>` — make the `$(…)` stdout capture cap configurable; `MAX_CAPTURED` is hardcoded at 64 MiB (`cmd_subst.rs:13`); bash has no such limit, so this is an fdshell-specific escape hatch for scripts that legitimately capture more than the default
- [ ] `recv_fd` pid verification is best-effort — SCM_CREDENTIALS checked only if delivered; make mandatory (`shellfd/recv_fd.rs`)
- [ ] `FDSHELL_PID`/`FDSHELL_SOCKET` trust — wrapper can spoof nested-shell env and capture exported fds (`init.rs`); document/limit trust boundary
- [ ] `~` / `$HOME` escape the capability model — the shell operates on fd-vars (`%CWD`) but `~` expansion (`substitute/arg.rs:24`) and `cd_home` (`cd/mod.rs:20`) open the inherited `$HOME` via *absolute* path with default `openat2` flags (no `RESOLVE_BENEATH`, no `O_NOFOLLOW`); a symlink at `$HOME` (or inside it) silently redirects file ops / `cd` to an attacker-controlled location, and `~` reaches outside any `RESOLVE_BENEATH` sandbox. Resolve `~` against a controlled dirfd, or drop `~` in strict mode

### P2 — Hardening / informational

- [x] Numbered path redirects at `i32::MAX` (`true 2147483647>file`) fail at `dup2` with a generic "failed to open redirection path" / `EBADF` (verified: `true 2147483647>%f` gives the clean "file descriptor number is out of range" but the path branch does not) — apply the same range check as the var branch (`redirect/resolve.rs:24-25` is var-only; add it for `RedirectSource::Path` in `resolve_redirects` or range-check `export_to` at parse time in `parse/redirect.rs:34`)
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
- [ ] Provenance/audit: tag every fd with origin; `fdexplain %foo` → "opened by openat2 from %CWD, line 3" — design is reusable from string provenance: `Origin`/`Trace`/`ScriptText` in `unsafe/sys/src/importedstr/` already track `(Position, Origin)` for strings and the `explain` builtin renders them; attach a `Trace` to fd vars at acquisition points and add an `fdexplain` builtin (done: `FdVar { fd, trace }` in `state.rs`, `fdexplain` builtin)
- [ ] Env-var provenance: `assign_origin` (`run_origin.rs:26`) only looks up `state.strings`, but env vars live in `state.environ` — `BAZ=$FOO` with FOO inherited from the environment gets the *line's* origin (e.g. `argv[2]`), not `EnvVar(FOO)`; and `explain FOO` reports "unset" for env-only vars (the `explain` builtin also never consults the environ). Consult the environ in both so env origins propagate like other transitive assignments
- [ ] Leak-detector test mode: snapshot `/proc/self/fd` before/after script run, assert no stragglers; verify CLOEXEC invariants

### P3 — Application domains (emerge from above)

- [ ] Init/supervision: pidfds + readiness tags + `wait --any` + restart policies
- [ ] Mini container runtime: namespaces + new mount API + landlock + seccomp, orchestrated in script
- [x] Busybox-style multicall binary: builtins symlinked as cat, mv, ls via getdents64 on dirfd

### P3 — Engineering / ecosystem

- [ ] aarch64/riscv64 ports (sys crate already isolates syscall numbers); static musl builds
- [ ] Kernel feature detection with documented degradation (openat2 needs 5.6+, pidfds 5.3+)
- [ ] Parser fuzzing
- [ ] ShellCheck-style linter (fd leaks, unset-in-branch, missing wait)
- [ ] "Writing TOCTOU-free scripts" guide
