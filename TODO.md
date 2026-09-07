# TODO

## Bash compatibility gaps

### P0 — Easy wins

- [x] `set -x` / `set +x` (xtrace) — `XTRACE` bit in `options.rs`, `xtrace.rs` prints `+ <name> <args>` to stderr at dispatch entry; open follow-up: a mode that traces unresolved references (`$VAR` / `%fd`) instead of expanded values, which can carry secrets into stderr

### P2 — Important for bash compatibility (hard)

- [ ] Heredocs (`<<EOF`) — multi-line stdin from script body with delimiter scanning
- [ ] Glob expansion (`*`, `?`, `[...]`) — expand patterns to matching filenames
- [ ] Arithmetic expansion `$((expr))` — integer expression evaluation
- [ ] Brace expansion (`{a,b,c}`, `{1..5}`) — pre-tokenization string generation
- [ ] Job control — `bg`, `fg`, `jobs`, SIGTSTP handling, TTY pgrp management
- [ ] Process substitution (`<(cmd)`, `>(cmd)`) — fifo/pipe with /dev/fd path
- [ ] `"$@"` preservation — expand to multiple words preserving empty args
- [ ] History expansion (`!!`, `!echo`) — readline-style history

## Refactoring

- [ ] Files in the 80-90 line zone (STYLE.md §2.3): `parse/wait_block/pattern.rs` (90), `intercept/set_list.rs` (89), `child/statx/parse.rs` (89), `intercept/hash_cmd.rs` (88), `comment.rs` (88), `repl.rs` (88), `parse/token/step.rs` (86), `parse/if_block.rs` (82), `launch.rs` (81), `intercept/validation.rs` (81), `parse/mod.rs` (81), `child/dispatch.rs` (81), `parse/wait_block.rs` (80), `intercept/ulimit_cmd/parse.rs` (80), `child/test/filetest.rs` (84), `sys/lib.rs` (81), `child/fdops/parse.rs` (82)
- [ ] `replacer.rs` `builtin_first` branch duplicates the substitute → seal → trace → dispatch pattern of the `builtin` keyword branch (`replacer.rs:44-59` vs `child/run.rs:36-42`) — extract a shared helper

## Security / hardening

### P1 — DoS / hardening

- [ ] `FDSHELL_PID`/`FDSHELL_SOCKET` trust — wrapper can spoof nested-shell env and capture exported fds (`init.rs`); document/limit trust boundary
- [ ] Pipeline builtin children hold every pipe/socketpair/pidfd open — `pipeline/mod.rs:27-57` copies `pipes`/`capture_pairs` into each forked child as borrows that are never dropped; external-command exec hides this via CLOEXEC, but builtins run their whole lifetime with the full inheritance (verified via `/proc/<pid>/fd`: a stage-3 builtin in a 3-stage pipeline holds ~15 fds, incl. sibling pipe ends, capture socketpair, sibling pidfds). Latent risk: any long-running/blocking builtin mid-pipeline keeps upstream write ends alive → upstream readers never see EOF → pipeline deadlock. In the child, before running a builtin, close all pipe ends except the two cloned ones and drop sibling capture pairs/pidfds
- [ ] `~` / `$HOME` escape the capability model — the shell operates on fd-vars (`%CWD`) but `~` expansion (`substitute/arg.rs:24`) and `cd_home` (`cd/mod.rs:20`) open the inherited `$HOME` via *absolute* path with default `openat2` flags (no `RESOLVE_BENEATH`, no `O_NOFOLLOW`); a symlink at `$HOME` (or inside it) silently redirects file ops / `cd` to an attacker-controlled location, and `~` reaches outside any `RESOLVE_BENEATH` sandbox. Resolve `~` against a controlled dirfd, or drop `~` in strict mode

### P2 — Hardening / informational

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

- [ ] Landlock syscall wrappers + builtin (`landlock --allow-rw %src --restrict`)
- [ ] fs-verity ioctls (verify binary before execveat)
- [x] `getdents64` syscall wrapper + builtin — list a directory by dirfd; the foundation for TOCTOU-free glob expansion (feeds the P2 glob item)
- [x] `memfd` builtin — heredocs without temp files, sealed secrets by fd (wrapper exists in `unsafe/sys/src/memfd.rs`; add `F_SEAL_*` / `memfd_set_seal` support and a name/size argument)
- [ ] `sendmsg` / `recvmsg` builtins — raw AF_UNIX payload + SCM_RIGHTS fd transfer for *custom* protocols (the existing wrappers are FDSHELL-protocol-only, `shellfd/send_fd.rs` / `shellfd/recv_fd.rs`): send a byte payload (arg or file fd) plus any number of fd vars; receive a payload into a var plus N named fd vars (script declares the slots); optional cred surfacing (SO_PASSCRED + pid/uid into a var) so custom protocols can enforce the same pid-verification rule as the `recv_fd` item
- [ ] Socket lifecycle builtins on fd vars — `bind` (socket + bind), `listen` (socket + bind + listen, backlog arg, `--type stream|dgram`; AF_UNIX with a path or abstract-namespace address (`@…` — no filesystem socket file, no path TOCTOU, fits the capability model; define who unlinks a filesystem socket path), or AF_INET via `--bind ADDR --port N`), `accept` (blocking accept on a listening fd var → new fd var, captured with the existing `%>%var` form); `accept %fd %>%array[N]` — bounded-capture form: if the array is full, accept and close immediately (reject; RST if unread data is buffered — acceptable for a cap), bounding the concurrent `wait` arm children, the real unbounded resource; the event-loop form of the same cap + parent-side append is the implemented bounded capture (`%>%arr[N]` / `%tag>%arr[N]`, `readable %listener` arm); `accept` blocks, so it is a top-level command (mid-pipeline it hits the pipeline fd-leak item above)
- [ ] `setsockopt` builtin — named options on an existing fd var (`PASSCRED`, `PASSFD`, …), replacing the hardcoded `SO_PASSCRED` helper in `net.rs:5-12`
- [ ] `splice` / `copy_file_range` / `sendfile` builtins → zero-copy cat/cp between fd vars, no path re-lookup on the hot path
- [ ] `O_TMPFILE` + `linkat` atomic file creation — write to an unlinked tempfd, `linkat` into the target dirfd only when complete, so the target path is never observable half-written (needs `linkat` wrapper + builtin)
- [ ] `openat2 --path` (O_PATH) — hold a handle to a file without open permission; combine with `fstat` / `fchdir` / `faccessat2` for inspect-then-act on files the user may not be able to read (`O_PATH` is now a named `--flags` constant, so `--flags O_PATH|O_NOFOLLOW` works; the `--path` shorthand remains)
- [ ] `test` fdshell extras beyond bash: `-fdsize +/-N` (size compare); `openat2 --same-as %fd` (verify inode at open time instead of a separate test step)
- [ ] `mkfifoat` syscall wrapper + builtin — create a fifo inside a dirfd var; underpins coprocess / message-passing scripts without temp files

### P1 — Language features

- [ ] Lexical scoping / RAII for fd vars: auto-close at block end, linear-use check (use-after-unset + leaks as parse-time errors)
- [ ] fd-var arrays — `%arr=[]` (empty), `%arr+=%conn` (append; dup semantics, matching `%var1=%var2`), `unset %arr[%conn]` (remove the entry originating from `%conn` — match by provenance, since a dup'd entry has a different fd number; inode match via `-fdeq` is ambiguous when one conn was added twice), `unset %arr` (close every descriptor the array owns); indexed read-out `%x=%arr[N]`; iteration by extending the word list of the existing `for %x in …` (`parse/for_block.rs`) to take an array ref, expanding each entry into `%x` as a dup; pairs with `accept` (hold all connections, fan out) and typed fd vars (one array, one kind)
- [ ] Bounded array capture `%>%arr[N]` / `%tag>%arr[N]` — general extension of fd capture (`parse/capture.rs`, `capture.rs`): received fds append to an array var up to N elements, beyond the cap closed (accept use: RST if unread data buffered); `%>` untagged (any tag), `%tag` matching tag only, as in the existing single-fd forms; usable by any command — `wait` arms build on it (implemented); decomposed via the for-loop-over-arrays item above
- [x] `wait` keyword — event-case over fd vars: one poll round, fork-per-arm, keep/release via the arm's exit status, `readable` / `writable` / `finished` / `after N` arms, `%arr[]` wildcard, bounded capture (`%>%arr[N]`), matched fd bound to `%?`; the legacy one-shot `wait %p1` is now the `waitpid` builtin (`intercept/waitpid.rs`); supersedes the `poll`/`epoll` builtin idea and subsumes the `wait --any` + `--timeout` half of the `timerfd` item. Remaining:
  - [ ] async reaping + reentrancy guard — v1 reaps each arm child synchronously before the block returns; the target model lets an arm child outlive the poll round (its pidfd joins the next round's set for non-blocking reaping) and keeps an fd with a live arm child out of the poll set meanwhile (kernel buffers the data)
  - [ ] `break` in an arm body exits the child, not the parent's `while` loop — propagate via the block's `$?` or an explicit sentinel
  - [ ] the poll wait must loop on `EINTR` once signal handlers can interrupt it (the `signalfd` item); removed for now as dead code (no handlers installed)
  - [ ] validate the echo-server proof-of-concept end-to-end once `listen` / `accept` land (the socket-lifecycle item above)
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

- [ ] Init/supervision: pidfds + readiness tags + the `wait` keyword + restart policies
- [ ] Mini container runtime: namespaces + new mount API + landlock + seccomp, orchestrated in script

### P3 — Engineering / ecosystem

- [ ] aarch64/riscv64 ports (sys crate already isolates syscall numbers); static musl builds
- [ ] Kernel feature detection with documented degradation (openat2 needs 5.6+, pidfds 5.3+)
- [ ] Parser fuzzing
- [ ] `wait` arm-parser paths show 0% in `nix build .#coverage` — `parse/wait_block/pattern.rs` `after()` / `arm()` / `parse_fdref()` / `parse_captures()` (incl. the `WaitInvalidTimeout` line ~63) report no hits: the 14 `wait::tests::*` unit tests run and pass, but the coverage checkPhase doesn't attribute bin-target unit-test coverage, so only integration tests drive the numbers for these three files. Add a process-level `safe/fdshell/tests/wait.rs` integration test driving `wait … readable` / `wait … after <ms>` scripts through the binary (mirroring `tests/signalfd.rs` / `tests/timeout.rs`, the files that cover the changed lines of their parsers), or fix the coverage checkPhase to include bin unit tests
- [ ] ShellCheck-style linter (fd leaks, unset-in-branch, missing wait)
- [ ] "Writing TOCTOU-free scripts" guide
