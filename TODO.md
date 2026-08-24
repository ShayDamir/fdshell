# TODO

## Bash compatibility gaps

### P0 — Easy wins

- [ ] `$-` — shell option flags
- [ ] `set -o ignoreeof` / `set +o ignoreeof` — prevent shell from exiting on EOF (`Ctrl+D`), P0; add `IGNOREEOF` shell option (`options.rs`); default `off` for scripts, `on` for interactive use; pairs with the `$-` item (shows `i` when on)
- [ ] `type` builtin — show command type (builtin, external, fd var, etc.)
- [ ] `command` builtin — bypass function lookup (alias for `builtin` prefix)
- [ ] `set` listing — bare `set` lists all variables (bash compat; currently falls through to external lookup, `set_cmd.rs:16-26`); add a flag to list fd variables — name TBD, avoid `-f` (bash reserves it for `noglob` when glob expansion lands)
- [ ] `set -x` / `set +x` (xtrace) — new `XTRACE` bit in `options.rs`; print `+ <name> <expanded args>` to stderr (PS4 prefix, default `+ `) at each dispatch entry — intercept commands, `replacer.rs`, pipeline stages — before the command runs; pairs with the `$-` item (shows `x` when on); security: expanded values can carry secrets into stderr — consider a mode that traces unresolved references (`$VAR` / `%fd`) instead
- [ ] Builtin-first lookup — `set -o builtin_first` option (`options.rs`): resolve bare command names against the builtin table (`is_dispatched`, `child/dispatch.rs:40`) before PATH lookup (`replacer.rs:44-45`), making the `builtin` keyword optional and retiring the hardcoded auto-builtin list (`parse/builtin.rs:3`); PATH can no longer swap which `test` / `printf` / `openat2` runs; explicit `/path/...` still reaches externals; candidate default under strict mode (P3)

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

- [ ] `timerfd` syscall wrapper + builtin (the `wait --any` + `--timeout` half is subsumed by the `wait` keyword, [TODO-WAIT.md](TODO-WAIT.md))
- [ ] `signalfd` builtin — traps as another fd source
- [ ] `eventfd` builtin — counters between background tasks
- [ ] `timeout N` builtin — run a command with a wall-clock limit; after N seconds signal the child (SIGTERM, then SIGKILL) and fail the command; pairs with the `timerfd` item above
- [ ] Landlock syscall wrappers + builtin (`landlock --allow-rw %src --restrict`)
- [ ] `pidfd_send_signal` builtin — kill background jobs by pidfd var
- [ ] fs-verity ioctls (verify binary before execveat)
- [ ] `getdents64` syscall wrapper + builtin — list a directory by dirfd; the foundation for TOCTOU-free glob expansion (feeds the P2 glob item)
- [ ] `memfd` builtin — heredocs without temp files, sealed secrets by fd (wrapper exists in `unsafe/sys/src/memfd.rs`; add `F_SEAL_*` / `memfd_set_seal` support and a name/size argument)
- [ ] `flock` builtin — advisory locking on existing fd vars (`flock %lock --wait`); coordinate processes by handle, never by path
- [ ] `sendmsg` / `recvmsg` builtins — raw AF_UNIX payload + SCM_RIGHTS fd transfer for *custom* protocols (the existing wrappers are FDSHELL-protocol-only, `shellfd/send_fd.rs` / `shellfd/recv_fd.rs`): send a byte payload (arg or file fd) plus any number of fd vars; receive a payload into a var plus N named fd vars (script declares the slots); optional cred surfacing (SO_PASSCRED + pid/uid into a var) so custom protocols can enforce the same pid-verification rule as the `recv_fd` item
- [ ] Socket lifecycle builtins on fd vars — `bind` (socket + bind), `listen` (socket + bind + listen, backlog arg, `--type stream|dgram`; AF_UNIX with a path or abstract-namespace address (`@…` — no filesystem socket file, no path TOCTOU, fits the capability model; define who unlinks a filesystem socket path), or AF_INET via `--bind ADDR --port N`), `accept` (blocking accept on a listening fd var → new fd var, captured with the existing `%>%var` form); `accept %fd %>%array[N]` — bounded-capture form: if the array is full, accept and close immediately (reject; RST if unread data is buffered — acceptable for a cap), bounding the concurrent `wait` arm children, the real unbounded resource; the event-loop form of the same cap + parent-side append is bounded capture (`%>%arr[N]` / `%tag>%arr[N]`, `readable %listener` arm) in [TODO-WAIT.md](TODO-WAIT.md); `accept` blocks, so it is a top-level command (mid-pipeline it hits the pipeline fd-leak item above)
- [ ] `setsockopt` builtin — named options on an existing fd var (`PASSCRED`, `PASSFD`, …), replacing the hardcoded `SO_PASSCRED` helper in `net.rs:5-12`
- [ ] `ftruncate` / `lseek` / `fsync` builtins on existing fd vars (`lseek` wrapper exists in `rw.rs`; add `ftruncate` / `fsync` wrappers)
- [ ] `splice` / `copy_file_range` / `sendfile` builtins → zero-copy cat/cp between fd vars, no path re-lookup on the hot path
- [ ] `O_TMPFILE` + `linkat` atomic file creation — write to an unlinked tempfd, `linkat` into the target dirfd only when complete, so the target path is never observable half-written (needs `linkat` wrapper + builtin)
- [ ] `statx` builtin — metadata by dirfd + relative path, superseding the `stat` / `fstat` wrappers; `AT_SYMLINK_NOFOLLOW` for TOCTOU-safe symlink checks, re-stat the same open handle after open
- [ ] `readlinkat` builtin — resolve symlink targets by dirfd without escaping the resolution root (pairs with `statx` for symlink-safe open)
- [ ] `openat2 --path` (O_PATH) — hold a handle to a file without open permission; combine with `fstat` / `fchdir` / `faccessat2` for inspect-then-act on files the user may not be able to read
- [ ] `test`: every bash operator that takes a path also takes a `%fd` — fstat the fd var instead of stat'ing a path; the `%var` lookup exists for `-e -f -d` already (`child/test/mod.rs:57-61`), extend to the rest:
  - kinds: `-f -d -b -c -p -S` (st_mode); `-t` is fd-native in bash (`test -t 1`) — accept a fd var; `-L` only meaningful for O_PATH fds opened on a symlink
  - permissions: `-r -w -x -g -k` (mode bits vs uid/gid, or faccessat via `/proc/self/fd/N`)
  - size: `-s` (exists and size > 0)
  - binary: `%fd1 -nt %fd2` / `-ot` (mtime); `%fd1 -fdeq %fd2` / `-fdne` — same `(st_dev, st_ino)`, the fd version of bash's `-ef` (ino alone is ambiguous across filesystems; both pipe ends share one inode, so this also answers "same pipe?"); use case: TOCTOU guard — reopen a path, verify it still resolves to the same inode as a previously held fd var
  - fdshell extras beyond bash: `-fdsize +/-N` (size compare); `openat2 --same-as %fd` (verify inode at open time instead of a separate test step)
- [ ] `fallocate` syscall wrapper + builtin — preallocate space on a file fd var
- [ ] `mkfifoat` syscall wrapper + builtin — create a fifo inside a dirfd var; underpins coprocess / message-passing scripts without temp files

### P1 — Language features

- [ ] Lexical scoping / RAII for fd vars: auto-close at block end, linear-use check (use-after-unset + leaks as parse-time errors)
- [ ] fd-var arrays — `%arr=[]` (empty), `%arr+=%conn` (append; dup semantics, matching `%var1=%var2`), `unset %arr[%conn]` (remove the entry originating from `%conn` — match by provenance, since a dup'd entry has a different fd number; inode match via `-fdeq` is ambiguous when one conn was added twice), `unset %arr` (close every descriptor the array owns); indexed read-out `%x=%arr[N]`; iteration by extending the word list of the existing `for %x in …` (`parse/for_block.rs`) to take an array ref, expanding each entry into `%x` as a dup; pairs with `accept` (hold all connections, fan out) and typed fd vars (one array, one kind)
- [ ] Bounded array capture `%>%arr[N]` / `%tag>%arr[N]` — general extension of fd capture (`parse/capture.rs`, `capture.rs`): received fds append to an array var up to N elements, beyond the cap closed (accept use: RST if unread data buffered); `%>` untagged (any tag), `%tag` matching tag only, as in the existing single-fd forms; usable by any command — `wait` arms build on it ([TODO-WAIT.md](TODO-WAIT.md)); decomposed via the for-loop-over-arrays item above
- [ ] `wait` keyword — event-case over fd vars; full design in [TODO-WAIT.md](TODO-WAIT.md) (arms incl. `accept %listenfd %array limit N` with built-in cap, one-shot rounds, fork-per-arm, keep/release via arm exit status, one-shot `wait %p1` legacy form); absorbs the legacy `wait` builtin, supersedes the `poll`/`epoll` builtin idea, and subsumes the `wait --any` + `--timeout` half of the `timerfd` item
- [ ] Structured return channel: extend socket protocol to carry payloads (statx results, readlink targets, error strings) alongside fds

### P2 — Syscall coverage

- [ ] `FICLONE` ioctl for reflinks
- [ ] More `*at` coverage: `symlinkat`, `utimensat` builtins (unlinkat syscall wrapper exists, no builtin)

### P2 — Language features

- [ ] Typed fd vars: dir vs file vs pipe-end vs socket vs pidfd, checked against builtin expectations
- [ ] Coprocesses and process substitution: `coproc name { cmd }` with bidirectional pipe fd vars
- [ ] Escape hatch expansion: `%var:path` → `/proc/self/fd/63` for programs that only accept paths

### P3 — Security directions

- [ ] Strict mode: ban absolute path resolution entirely; every operation relative to an explicit dirfd — capability shell in Capsicum spirit
- [ ] Broker pattern: make socket bidirectional so sandboxed child can request an open; privileged shell resolves against its dirfds
- [ ] Provenance/audit: tag every fd with origin; `fdexplain %foo` → "opened by openat2 from %CWD, line 3" — design is reusable from string provenance: `Origin`/`Trace`/`ScriptText` in `unsafe/sys/src/importedstr/` already track `(Position, Origin)` for strings and the `explain` builtin renders them; attach a `Trace` to fd vars at acquisition points and add an `fdexplain` builtin (done: `FdVar { fd, trace }` in `state.rs`, `fdexplain` builtin)
- [ ] Env-var provenance: `assign_origin` (`run_origin.rs:26`) only looks up `state.strings`, but env vars live in `state.environ` — `BAZ=$FOO` with FOO inherited from the environment gets the *line's* origin (e.g. `argv[2]`), not `EnvVar(FOO)`; and `explain FOO` reports "unset" for env-only vars (the `explain` builtin also never consults the environ). Consult the environ in both so env origins propagate like other transitive assignments
- [ ] Leak-detector test mode: snapshot `/proc/self/fd` before/after script run, assert no stragglers; verify CLOEXEC invariants

### P3 — Application domains (emerge from above)

- [ ] Init/supervision: pidfds + readiness tags + the `wait` keyword ([TODO-WAIT.md](TODO-WAIT.md)) + restart policies
- [ ] Mini container runtime: namespaces + new mount API + landlock + seccomp, orchestrated in script

### P3 — Engineering / ecosystem

- [ ] aarch64/riscv64 ports (sys crate already isolates syscall numbers); static musl builds
- [ ] Kernel feature detection with documented degradation (openat2 needs 5.6+, pidfds 5.3+)
- [ ] Parser fuzzing
- [ ] ShellCheck-style linter (fd leaks, unset-in-branch, missing wait)
- [ ] "Writing TOCTOU-free scripts" guide
