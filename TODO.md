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
- [x] IFS sync only happens on `IFS=…` assignment — `read IFS`, `export IFS=x`, `${IFS=…}`, `for IFS in …` store in `strings` without updating `state.ifs` (centralize on a `set_var` helper or sync at each insert site)
- [x] Unquoted `$@`/`$*` join positional args with literal spaces before IFS splitting — wrong with custom IFS lacking space (injected spaces survive) and with empty IFS (collapses to one field); split per-positional instead of join-then-split
- [x] `shopt` / `set -o` — shell options bitmask
- [x] Alias expansion — text-replacement pass on command words
- [x] Alias expansion only rewrites the first word of a line — words after `|`, `&&`, `||` (cond.rs/pipeline subcommands) are not alias-expanded
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

## Parser & expansion bugs

- [x] `$(…)` paren matching is quote-blind — `read_dollar_paren` (`parse/token_subst.rs:18-43`) and `read_paren_expr` (`substitute/paren.rs:6-35`) count raw `(`/`)` bytes without tracking double-quote or backslash state, so a `)` that is data inside `"…"` terminates the substitution early and the remainder of the line is re-parsed as shell syntax — text intended as data after the `)` becomes executable (injection class, e.g. `res=$(lookup "key:$user_input")` with a `)` in the input):
  ```
  fdshell -c 'x=$(echo "a)b"); echo got:$x'   # → parse error "unmatched quote"; bash prints got:a)b
  ```
  Track quote/backslash state in both scanners, or tokenize once and drive substitution from tokens instead of re-scanning raw text
- [ ] Word splitting cuts through quoted sections of mixed tokens — quote boundaries are erased during tokenize and "fully quoted" is one bit per token (`parse/token.rs:43-49`), so `split_word` (`substitute/mod.rs:41-47`) splits on IFS chars *inside* what was quoted; one argv entry silently becomes two (argument smuggling — callees can be made to act on attacker-chosen fragments):
  ```
  fdshell -c 'builtin printf "[%s]" x"a b"c'   # → [xa][bc]; bash/POSIX: [xa bc] (one word)
  ```
  Carry per-character quoting through to the splitter, or keep quote spans alongside the token like bash's word structure
- [ ] Backslash swallowed for all characters inside double quotes — `parse/quotes.rs:14-23` pushes only `X` for `\X`, so `"C:\temp"` → `C:temp` and regexes/format strings containing `\<char>` are silently corrupted:
  ```
  fdshell -c 'x="a\nb"; builtin printf "%s|\n" "$x"'   # → anb|; bash: a\nb (backslash retained)
  ```
  Preserve the backslash unless the next char is one of `"` `\` `$` `` ` `` newline
- [ ] `#` begins a comment mid-word — `parse/token.rs:58-65` does not require `#` to start a token: `builtin echo a#b` prints `a` (bash prints `a#b`); data containing `#` (colors, hostnames, filenames) changes meaning depending on position. Require start-of-token like POSIX shells
- [ ] Shrinking alias underflows the expansion delta — `alias_expand.rs:88`: `*delta += value.len() - (e - s)` underflows when an alias value is shorter than the word it replaces (e.g. `alias ab="z"`): panics in debug builds; in release the wrapped delta mispositions later expansions; an empty alias aborts the rest of the line with "expected command". Use `checked_`/`saturating_` arithmetic with an explicit error

## Refactoring

- [ ] `parse/token_subst.rs` and `substitute/paren.rs` duplicate the quote/backslash `$(…)` scanning automaton — extract a shared helper (a byte-consumption scan returning the substitution body) so the two scanners cannot drift
- [ ] `parse/command.rs` is 83 code lines (STYLE.md §2.3 flag zone) — extract the arg/capture/redirect loop into a helper
- [ ] `redirect/resolve.rs` is 81 code lines (STYLE.md §2.3 flag zone) — extract the per-source arm construction into helpers
- [ ] `child/test/ops.rs` is 88 code lines (STYLE.md §2.3 flag zone) — extract the file-test path/fd lookup into a helper
- [ ] `parse/token.rs` is 80 code lines (STYLE.md §2.3 flag zone) — extract the per-byte match arm into a helper
- [ ] `parse/mod.rs` is 83 code lines (STYLE.md §2.3 flag zone) — extract the keyword dispatch into a helper
- [ ] `parse/redirect.rs` is 85 code lines (STYLE.md §2.3 flag zone) — extract the `>>`/`<>` operator dispatch into a helper
- [x] `state.rs` is 88 code lines (STYLE.md §2.3 flag zone) — extract the setter cluster into a helper module
- [ ] `intercept/alias_cmd.rs` is 90 code lines (STYLE.md §2.3 flag zone) — extract the definition parsing into a helper
- [ ] `alias_expand.rs` is 89 code lines (STYLE.md §2.3 flag zone) — extract the per-position expansion loop into a helper

## Security / hardening

### P1 — DoS / hardening

- [ ] `set --stdout-capture-limit <bytes>` — make the `$(…)` stdout capture cap configurable; `MAX_CAPTURED` is hardcoded at 64 MiB (`cmd_subst.rs:13`); bash has no such limit, so this is an fdshell-specific escape hatch for scripts that legitimately capture more than the default
- [ ] `recv_fd` pid verification is best-effort — SCM_CREDENTIALS checked only if delivered; make mandatory: `ensure!(got_pid.is_some())` so an fd is never accepted without kernel-attested credentials (`shellfd/recv_fd.rs`)
- [ ] `FDSHELL_PID`/`FDSHELL_SOCKET` trust — wrapper can spoof nested-shell env and capture exported fds (`init.rs`); document/limit trust boundary
- [ ] `source` recursion bypasses MAX_NESTING → stack overflow + core dump — `run_sourced` (`intercept/source.rs`) recurses through `run_script` without `nest::deeper` (the cap of 100 guards only if/while/until/for/case/`$()`), so self-sourcing overflows the native stack:
  ```
  printf 'source /tmp/selfsrc.sh\n' > /tmp/selfsrc.sh
  fdshell -c 'source /tmp/selfsrc.sh'   # → thread 'main' has overflowed its stack (SIGABRT, core dumped)
  ```
  Wrap `run_sourced`'s `run_script` call in `nest::deeper` (and `eval_cmd::run_eval` for symmetry)
- [ ] Pipeline builtin children hold every pipe/socketpair/pidfd open — `pipeline/mod.rs:27-57` copies `pipes`/`capture_pairs` into each forked child as borrows that are never dropped; external-command exec hides this via CLOEXEC, but builtins run their whole lifetime with the full inheritance (verified via `/proc/<pid>/fd`: a stage-3 builtin in a 3-stage pipeline holds ~15 fds, incl. sibling pipe ends, capture socketpair, sibling pidfds). Latent risk: any long-running/blocking builtin mid-pipeline keeps upstream write ends alive → upstream readers never see EOF → pipeline deadlock. In the child, before running a builtin, close all pipe ends except the two cloned ones and drop sibling capture pairs/pidfds
- [ ] `~` / `$HOME` escape the capability model — the shell operates on fd-vars (`%CWD`) but `~` expansion (`substitute/arg.rs:24`) and `cd_home` (`cd/mod.rs:20`) open the inherited `$HOME` via *absolute* path with default `openat2` flags (no `RESOLVE_BENEATH`, no `O_NOFOLLOW`); a symlink at `$HOME` (or inside it) silently redirects file ops / `cd` to an attacker-controlled location, and `~` reaches outside any `RESOLVE_BENEATH` sandbox. Resolve `~` against a controlled dirfd, or drop `~` in strict mode

### P2 — Hardening / informational

- [x] Numbered path redirects at `i32::MAX` (`true 2147483647>file`) fail at `dup2` with a generic "failed to open redirection path" / `EBADF` (verified: `true 2147483647>%f` gives the clean "file descriptor number is out of range" but the path branch does not) — apply the same range check as the var branch (`redirect/resolve.rs:24-25` is var-only; add it for `RedirectSource::Path` in `resolve_redirects` or range-check `export_to` at parse time in `parse/redirect.rs:34`)
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
