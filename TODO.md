# TODO

## Bash compatibility gaps

### P0 — Easy wins

- [ ] `$-` — shell option flags
- [ ] `type` builtin — show command type (builtin, external, fd var, etc.)
- [ ] `command` builtin — bypass function lookup (alias for `builtin` prefix)

- [ ] `\$` inside double quotes expands instead of deferring — `builtin echo [\$_]` prints `[\hello]` (backslash retained AND `$_` expanded; bash prints literal `[$_]"). Consequence: nothing can defer a `$` reference into an `eval`/`source` body, so end-to-end tests cannot exercise the `eval_depth` gating (unit tests in `state/tests.rs` cover it directly). Fix escape handling first, then add an end-to-end test with `true hello; eval "true x y; builtin echo [\$_]"`

### P1 — Major functionality gaps (moderate effort)

- [ ] `hash` — PATH lookup cache
- [ ] `ulimit` — resource limit get/set

### P2 — Important for bash compatibility (hard)

- [ ] Heredocs (`<<EOF`) — multi-line stdin from script body with delimiter scanning
- [ ] Glob expansion (`*`, `?`, `[...]`) — expand patterns to matching filenames
- [ ] Arithmetic expansion `$((expr))` — integer expression evaluation
- [ ] Brace expansion (`{a,b,c}`, `{1..5}`) — pre-tokenization string generation
- [ ] Job control — `bg`, `fg`, `jobs`, SIGTSTP handling, TTY pgrp management
- [ ] Process substitution (`<(cmd)`, `>(cmd)`) — fifo/pipe with /dev/fd path
- [ ] `"$@"` preservation — expand to multiple words preserving empty args
- [ ] History expansion (`!!`, `!echo`) — readline-style history

## Parser & expansion bugs

- [ ] Word splitting cuts through quoted sections of mixed tokens — quote boundaries are erased during tokenize and "fully quoted" is one bit per token (`parse/token.rs:43-49`), so `split_word` (`substitute/mod.rs:41-47`) splits on IFS chars *inside* what was quoted; one argv entry silently becomes two (argument smuggling — callees can be made to act on attacker-chosen fragments):
  ```
  fdshell -c 'builtin printf "[%s]" x"a b"c'   # → [xa][bc]; bash/POSIX: [xa bc] (one word)
  ```
  Carry per-character quoting through to the splitter, or keep quote spans alongside the token like bash's word structure

## Refactoring

- [ ] `ShortCStr` has no byte-search API — several call sites do `as_bytes().ok().and_then(|b| b.iter().position(…))` instead (STYLE.md §6.4): `intercept/alias_cmd/args.rs:14` (`position(|&c| c == b'=')`), `parse/redirect.rs:40` (`position(|&b| b == b'>' || b == b'<')`), `busybox.rs:17` (`rposition(|&c| c == b'/')`). Add `find_byte(byte: u8) -> Option<usize>` next to `contains` in `unsafe/sys/src/shortcstr/eq.rs` (plus `rfind_byte` / a byte-set variant if the other sites need them), cover with tests in `unsafe/sys/tests/`, and switch all call sites off `as_bytes()`

## Security / hardening

### P1 — DoS / hardening

- [ ] `set --stdout-capture-limit <bytes>` — make the `$(…)` stdout capture cap configurable; `MAX_CAPTURED` is hardcoded at 64 MiB (`cmd_subst.rs:13`); bash has no such limit, so this is an fdshell-specific escape hatch for scripts that legitimately capture more than the default
- [ ] `recv_fd` pid verification is best-effort — SCM_CREDENTIALS checked only if delivered; make mandatory: `ensure!(got_pid.is_some())` so an fd is never accepted without kernel-attested credentials (`shellfd/recv_fd.rs`)
- [ ] `FDSHELL_PID`/`FDSHELL_SOCKET` trust — wrapper can spoof nested-shell env and capture exported fds (`init.rs`); document/limit trust boundary
- [ ] Pipeline builtin children hold every pipe/socketpair/pidfd open — `pipeline/mod.rs:27-57` copies `pipes`/`capture_pairs` into each forked child as borrows that are never dropped; external-command exec hides this via CLOEXEC, but builtins run their whole lifetime with the full inheritance (verified via `/proc/<pid>/fd`: a stage-3 builtin in a 3-stage pipeline holds ~15 fds, incl. sibling pipe ends, capture socketpair, sibling pidfds). Latent risk: any long-running/blocking builtin mid-pipeline keeps upstream write ends alive → upstream readers never see EOF → pipeline deadlock. In the child, before running a builtin, close all pipe ends except the two cloned ones and drop sibling capture pairs/pidfds
- [ ] `~` / `$HOME` escape the capability model — the shell operates on fd-vars (`%CWD`) but `~` expansion (`substitute/arg.rs:24`) and `cd_home` (`cd/mod.rs:20`) open the inherited `$HOME` via *absolute* path with default `openat2` flags (no `RESOLVE_BENEATH`, no `O_NOFOLLOW`); a symlink at `$HOME` (or inside it) silently redirects file ops / `cd` to an attacker-controlled location, and `~` reaches outside any `RESOLVE_BENEATH` sandbox. Resolve `~` against a controlled dirfd, or drop `~` in strict mode

### P2 — Hardening / informational

- [ ] Non-CLOEXEC socket fd leaks into nested-shell grandchildren via `FDSHELL_SOCKET` — ensure CLOEXEC or strip in children
- [ ] `ExportedCStr::as_ref` uses `unreachable_unchecked` and `CStr::from_bytes_with_nul_unchecked` (`shortcstr/mod.rs:72-89`); `shortcstr/push.rs:43-108` uses `get_unchecked_mut` and a transmute-based `InlineSize` guarded only by `debug_assert!`; tail-`Static` `as_cstr_bytes` ignores `length` — sound under current construction invariants but UB-fragile to future edits; replace `unreachable_unchecked` with the existing `ShortCStrError::BadState` mapping and add `verify()` coverage per STYLE.md §7.4
- [ ] Capture completion depends on socket EOF from *all* socket holders — `do_captures` (`capture.rs:38-68`) loops `recv_fd` until EOF; EOF requires every holder of the child-end to exit, including descendants that inherited the exported socket dup (no CLOEXEC by definition of `ExportedFd`); a descendant outliving its child delays capture past `wait_pidfd`, and for background tasks (`pidvar` path) the same stall hits the `wait` builtin. Add a timeout, or count expected senders explicitly
- [ ] Unbounded script size — `cli::load_script` and the `-c`/stdin paths read the entire script into a `Vec<u8>` with no cap (`cli.rs:7`); a multi-GB script / `-c` argument OOMs the shell before parsing (compounds with the nested-`if` O(n²) CPU). `source` targets are read the same way (`read_to_end` in `intercept/source.rs`): `source /dev/zero` allocates until OOM as an exit-free memory bomb. Reuse the `MAX_CAPTURED` cap + error for both loaders
- [ ] Error output leaks absolute internal source paths — error reports embed locations like `at safe/fdshell/src/exec/search.rs:19:5` on stderr; build-path leakage in dev/test builds — strip via release profile or route through the display chain only
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
- [ ] `getdents64` syscall wrapper + builtin — list a directory by dirfd; the foundation for TOCTOU-free glob expansion (feeds the P2 glob item)
- [ ] `memfd` builtin — heredocs without temp files, sealed secrets by fd (wrapper exists in `unsafe/sys/src/memfd.rs`; add `F_SEAL_*` / `memfd_set_seal` support and a name/size argument)
- [ ] `flock` builtin — advisory locking on existing fd vars (`flock %lock --wait`); coordinate processes by handle, never by path
- [ ] `ftruncate` / `lseek` / `fsync` builtins on existing fd vars (`lseek` wrapper exists in `rw.rs`; add `ftruncate` / `fsync` wrappers)
- [ ] `splice` / `copy_file_range` / `sendfile` builtins → zero-copy cat/cp between fd vars, no path re-lookup on the hot path
- [ ] `O_TMPFILE` + `linkat` atomic file creation — write to an unlinked tempfd, `linkat` into the target dirfd only when complete, so the target path is never observable half-written (needs `linkat` wrapper + builtin)
- [ ] `statx` builtin — metadata by dirfd + relative path, superseding the `stat` / `fstat` wrappers; `AT_SYMLINK_NOFOLLOW` for TOCTOU-safe symlink checks, re-stat the same open handle after open
- [ ] `readlinkat` builtin — resolve symlink targets by dirfd without escaping the resolution root (pairs with `statx` for symlink-safe open)
- [ ] `openat2 --path` (O_PATH) — hold a handle to a file without open permission; combine with `fstat` / `fchdir` / `faccessat2` for inspect-then-act on files the user may not be able to read

### P1 — Language features

- [ ] Lexical scoping / RAII for fd vars: auto-close at block end, linear-use check (use-after-unset + leaks as parse-time errors)
- [ ] Structured return channel: extend socket protocol to carry payloads (statx results, readlink targets, error strings) alongside fds

### P2 — Syscall coverage

- [ ] `FICLONE` ioctl for reflinks
- [ ] More `*at` coverage: `symlinkat`, `utimensat` builtins (unlinkat syscall wrapper exists, no builtin)

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

### P3 — Engineering / ecosystem

- [ ] aarch64/riscv64 ports (sys crate already isolates syscall numbers); static musl builds
- [ ] Kernel feature detection with documented degradation (openat2 needs 5.6+, pidfds 5.3+)
- [ ] Parser fuzzing
- [ ] ShellCheck-style linter (fd leaks, unset-in-branch, missing wait)
- [ ] "Writing TOCTOU-free scripts" guide
