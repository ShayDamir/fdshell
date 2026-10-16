# Lessons Learned

## Recursive glob_match caused stack overflow on long inputs
`match_glob` used two recursive calls — star backtracking loop + literal-character match path. 10K literal chars = 10K frames. Converted to iterative star-backtracking (DFA-style with `star_pi`/`star_si` pointers). When a function recurses linearly on input length, prefer iterative with explicit backtrack state.

## `input.get(pos)` is ambiguous when `pos` comes from a trait object
`pos` from a trait method returning `usize` makes `input.get(pos)` ambiguous (index vs slice). Fix: extract `pos` to local, use `input.get(pos..)` or `input.get(..pos)`.

## `Iterator::rposition` returns index from start, not from end
`rposition` consumes from the right but returns index from the **start**. Verify with a small test.

## `cargo fmt` reformatting can change test formatting
Run `cargo fmt` before `nix flake check`.

## Always show the caret, even at position 0
Remove the `!(is_first_line && caret_col == 0)` guard. Fish shows `^^` at position 0; users benefit from the marker.

## Use `Iterator::position` with `skip` instead of `find` + tuple destructuring
`.skip(N).position(|pred)? + N` is cleaner than `.skip(N).find(|(_, t)| pred).ok_or(...)?.0`.

## Don't store redundant length constants in const arrays
Don't pair byte strings with `.len()` values — `kw.len()` is always available. Avoids maintenance burden.

## `Result::unwrap_err()` requires `Debug` on the `Ok` type
Use `match result { Ok(_) => panic!(), Err(e) => e }` when the Ok type lacks `Debug`.

## `error-stack` 0.7 uses `current_context()`, not `as_context()`
Use `.current_context()` to extract the context from `Report<T>`.

## `Display` on `ShortCStr` is correct for user-facing error messages; forbidden for data paths
Non-UTF-8 bytes use `from_utf8_lossy` — correct for errors. For data paths, use `as_bytes()`/`as_c_str()` (lossless).
`report_*` functions that know the position must attach the input line alongside `ParsePosition`.

## `.change_context()` with same-message variants causes duplicate Display output
Outer variant should describe *what* failed, inner describes *why*. Avoid "Y → because Y" chains.

## `dispatch_builtin` exit codes belong in `Ok`, not `Err`
Exit codes (0/1/errno) are shell convention, not errors. `Ok(code)`, only `Err(BuiltinError::Unknown)` on failure.

## `become` is a reserved keyword in Rust — rename modules
Rename `mod become` to `mod become_cmd` (file: `become_cmd.rs`). Only the module identifier matters.

## Prefer `Result::is_ok_and()` over `map().unwrap_or(false)`
`result.is_ok_and(|v| pred(v))` vs `result.map(|v| pred(v)).unwrap_or(false)`.

## Keep execution-context counters in ShellState, not threaded parameters
A limit that must be visible to both the block executors and `$(…)` (the `nesting` depth cap) should live in `ShellState`: a forked `$(…)` child inherits the state copy, so the counter propagates for free. Threading a parameter would have forced it through `substitute_arg` and its many callers.

## Clamp a `break`/`continue` count at the call site, not at the top-level handler
`loop_depth` lives in `ShellState` (the `nesting`/`eval_depth` precedent), so a `break`/`continue` at the call site knows the loop nesting depth and can clamp `n` to it. bash clamps silently (`break 2` in depth 1 and `break 5` in depth 1 both exit that one loop with no warning — measured), which the top-level handler cannot reproduce because by the time a leftover count reaches `repl/control.rs` the loop levels are gone. Clamping at the site makes `repl/control.rs`'s `Break(_)`/`Continue(_)` outside-loop arms unreachable (depth 0 bails in `run_simple`), and `cargo mutants` kills all of `repl/control.rs`'s mutants through the `BreakOutsideLoop`/`ContinueOutsideLoop` pins (measured with the whole `repl.rs` split of task #140, when these arms moved out of `repl.rs`), so the dead arms are not an accepted-equivalent-mutant source. See `run_dispatch.rs` (`n.min(&depth)`) and `loop_control.rs` (`in_loop`, `at_level` — the map a loop level applies to a control raised inside it; task #205 renamed it from `consume` when the loop **condition** started consuming too).

## A cond-list run returns loop control — every consumer must bind it
`run_cond_list` and `run_script` return `Result<Option<LoopControl>, _>`, and a bare `?` propagates only the error: the `Some(Break(_))`/`Some(Continue(_))` is dropped on the floor. That is how a `break` written in a `while` condition became an infinite loop (task #205): the loop body sites consumed the control, while the three **condition** sites — `loop_.rs`'s condition and `if_exec.rs`'s `if` and `elif` conditions — ran it with `?` and re-tested. Rule: bind the `Option`. At a **loop** level map it with `loop_control::at_level` (`Break(1)`→stop this loop, `Continue(1)`→its next iteration, `n`→hand `n-1` outward, `Return`/`Exit`→unchanged) and treat a condition exactly like the body — the condition runs inside the loop's own level (`in_loop`), so consuming it there is what makes `break 2` in a nested condition exit both loops; an early exit must return through the loop body's `Ok(..)`, never past the `in_loop` restore (the unit pins assert `loop_depth` is restored). At a **transparent** level (`if`, `{ …; }`, `case`, a function-call frame, `time`) propagate the control whole and consume nothing: clamping already happened at the call site (see "Clamp a `break`/`continue` count at the call site"), so any level that consumes a count it does not own loses one loop. Conditions run errexit-exempt (`run_cond_list(…, exempt = true)`), so `Exit` never comes out of one and the `AtLevel::Outer(Exit)` arm is defensive — it is exercised through the body path (`set -e; while true; do false; done`), so it is not an accepted-equivalent-mutant source. A `continue` in a condition advances the loop to its next condition test, which is an infinite loop in bash too (`while continue; do …; done`): never pinned, and every integration pin of a loop condition runs under a bounded child (`tests/loops.rs::run_bounded`, 5 s, strictly under nextest's 10 s `slow-timeout`) so a regression panics with the script in its message instead of hanging the suite.

## Never hold a ForkCell borrow across a recursive call that re-borrows
`nest::deeper` increments the depth, drops the borrow, runs the body (which borrows the same cell), then re-borrows to decrement. Holding the `RefMut` across the body would clash with the body's own borrows.

## A RefCell borrow conflict can masquerade as an unrelated domain error
`nest::deeper` bailed with `NestingTooDeep` both when the depth limit was hit AND when `borrow_mut()` failed (a RefCell still borrowed). `substitute_args` held a shared `cell.borrow()` across the whole arg loop, so any `$(` command substitution inside a command arg hit `borrow_mut()`, got a borrow conflict, and was misreported as "nesting too deep." Fix: scope the borrow to the narrowest use (only the `$@`/`$*` branches needed `state.positional`). General rule: when a `borrow_mut()` fallback bails with a domain error, a held borrow elsewhere surfaces as that unrelated error and is hard to diagnose.

## `|` on disjoint bit flags produces equivalent mutants
`O_WRONLY | O_CREAT | O_TRUNC` mutates `|`→`^`, which is unkillable because the O_* flags are disjoint single bits (`|` ≡ `^` ≡ `+` on them). Sum the flags with `+` instead: same value, but `+`→`-`/`*` mutants change the value and are killable by an exact-value test. See `RedirectDirection::open_flags` and `memfd::CREATE_FLAGS` (the `MFD_CLOEXEC + MFD_ALLOW_SEALING` sum, pinned by `create_flags_yield_a_cloexec_sealable_memfd`).

## An `O_RDONLY` (== 0) flag is inert and makes a `|` flag expression unkillable
`O_RDONLY` is `0`, so `O_RDONLY | O_DIRECTORY` ≡ `O_DIRECTORY` but carries a mutable `|`: the `|`→`^` mutant is unkillable (disjoint bits, `0 ^ X` ≡ `0 | X`) and the `|`→`&` mutant (`0 & X = 0`) still opens the same object and passes the test. The fix is the same species as "redundant open flags: drop them" — drop the `0` flag and keep only the meaningful `O_DIRECTORY`. A single flag needs no operator, so there is no equivalent mutant to chase. If two *non-zero* flags must combine, sum them with `+` instead of `|` (LESSONS: "`|` on disjoint bit flags produces equivalent mutants") so the `+`→`-`/`*` mutants change the value and are killable.

## Redundant open flags on a just-created path: drop them
In `mkdirat_exec`, the openat2 call that re-opens the just-created directory used `(O_DIRECTORY | O_NOFOLLOW)`. Both flags are redundant: mkdirat guarantees the path is a real directory, so the open succeeds regardless. The `|`→`^` mutant was unkillable (disjoint bits, same value) and the `|`→`&` mutant (flags=0) was also unkillable (open still succeeds). The fix is to pass `0`: no flags means no redundant flags to mutate, and the code honestly reflects that the flags have no observable effect. If the flags ever do matter (e.g. to reject a symlink race), add them back with a test that exercises the failure mode.

## `while cond` with an invariant condition produces equivalent mutants
In `read_paren_expr`, `while depth > 0` was always true at the loop top (depth starts at 1 and the only path to 0 breaks immediately), so `>`→`>=` was unkillable. A loop whose exit is fully decided by `break`/`bail!` inside the body should be a bare `loop`; the redundant condition is both dead logic and a mutant source. See `parse/backtick.rs`.

## Redundant boundary ±1 produces equivalent mutants
In `tokens_to_for`, `get(in_pos + 1..do_idx - 1)` adjusted bounds the surrounding code already guaranteed: `find_preceded_by_semi` only returns a `do` preceded by `;`, and `trim_semi` strips that `;` anyway — so the trailing `- 1` was unkillable (`-`→`/` is identity). When a later step normalizes what an offset excludes, drop the offset. Likewise, scanning from `in_pos` instead of `in_pos + 1` is equivalent when the token at `in_pos` can never match the needle. The same pattern in `tokens_to_loop` (`done_idx - 1` on the body slice) was worse: `done` is *not* guaranteed to be `;`-preceded, so the offset silently dropped the last body token in `while …; do cmd arg done`. Dropping it fixed a real off-by-one, not just the mutant.

## Block-keyword length only seeds the scan start: a ±1 offset is equivalent
`keyword_block` computes `kw_len` (the opening keyword's length) and starts the block scan at `start + leading_ws + kw_len`. Because `keyword_delta` only reports a keyword at a real boundary (the next byte is whitespace or `;`), `kw_len` lands the scan start inside the whitespace after the keyword, and `scan_block` skips leading whitespace — so a ±1 shift (e.g. the `||`→`&&` mutant that sends `case`/`wait` down the `else` 5-char branch) is unobservable: the whole suite passes under it. Don't chase it with a test (no input makes the offset matter, since the char after the keyword is always whitespace/`;`); it's an accepted equivalent mutant. Splitting the shared `4` into two arms to dodge it trips `clippy::if_same_then_else`, and production `#[allow]` is forbidden, so the combined `||` form stays.

**The premise is "the length measures the keyword".** Score `kw_len` on the *prefix-stripped* word: with a `time [-p]` prefix in front of the keyword, testing `starts_with` on the raw run sends `if`/`for` down the 5-char `else` branch, so the scan starts 3 bytes *past* the keyword and skips the head of the condition. There the equivalence breaks — a nested keyword lives there: `time for fi in 1; do echo $fi; done` scanned the seed after the nested `fi` and closed the block at `done` (the unprefixed form closes at the nested `fi`). `segment/block/keyword/tests.rs` pins the prefixed scan to end exactly where the unprefixed one does (`+ time_prefix_len`), which makes the seed measurable.

## Re-deriving an already-set flag is dead code with equivalent mutants
In `depth_delta` (quote-aware block scan in `comment.rs`), a per-char `frag_quoted |= in_quote || in_backtick || depth > 0;` could never change the result: a fragment that contains any quote, backtick, or paren byte can never *exactly* match a keyword, so only the run-start state (caller mid-quote/mid-backtick) can taint a keyword-shaped fragment, and that is covered by the `frag_quoted` initialization. The dead line carried four replacement mutants of its own and masked the initialization's `||`→`&&` mutant (the tail re-tainted after the broken init). Drop the redundant derivation instead of chasing tests; the init is killable by a unit test that calls `scan_block` with `in_quote = true` and a closer inside the open quote. Same species: a delegating `Iterator::size_hint` override (`Counting` in `substitute/mask.rs`) is unobservable because no consumer consults the hint — six replacement mutants all survived; removing the override removed the source.

The `time_prefix_len`/`one_prefix_len` pair is the same species in *length* form: `one_prefix_len` returned `4 + ws`/`6 + ws + ws2` (it had already consumed the whitespace closing the prefix) and the caller's loop added its own whitespace step on top of it, a term that was always 0. The dead step masked the `4 + ws` mutants (`* ws` was rescued by the outer `+= ws`). Fix: the helper returns the prefix's own length and the loop owns the closing whitespace, so every term is load-bearing. Two more rules came out of it: derive the shared word lengths from the matched bytes (`const TIME_LEN: usize = b"time".len();`) so there is no integer literal for `cargo mutants` to move, and pin each whitespace term with an exact length (`time \t-p  ls` → 10).

## Cap test memory with a nextest wrapper script, not shell `ulimit`
`ulimit -v` in the shell caps cargo/rustc too, which need more VA than the tests. Nextest wrapper scripts (`.config/nextest.toml` with `experimental = ["wrapper-scripts"]`) wrap only each test binary: `prlimit --as=134217728 --`. An allocation blow-up (infinite-allocation mutants under cargo-mutants) then aborts that one test instead of tripping the host OOM killer, which was killing unrelated processes and corrupting whole mutant runs. The cap must stay above legitimate peak VA (this suite peaks well under 128MB). Nix builds need `util-linux` in `nativeBuildInputs` — stdenv has no `prlimit`, and its absence fails every test with exec ENOENT.

## Test fork-children must `sys::exit`, not `std::process::exit`
`std::process::exit` runs atexit handlers, which flush Rust stdio by taking its mutex. If the fork happened while another thread (the libtest main thread writing progress) held that mutex, the child inherits it locked and deadlocks in `exit` on the futex — the parent's `wait_pidfd` then blocks forever. Symptom: one random fork-based test times out, passes in isolation, different victims each run. Fix: exit test fork-children with `sys::exit` (raw `_exit`, skips atexit) — already required for production fork-children in `pipeline.rs`. Remaining caveat: running one test binary with `--test-threads>1` still races on other process-wide state (env, cwd); nextest avoids this by running each test in its own process.

## A forked test child swallows its own panics unless you `catch_unwind` them
A test helper that does `fork()` then runs the assertions in the child and exits 0 on the way out is silently wrong when an assertion panics: the child is a full copy of the process *including the running libtest harness*, whose per-test `catch_unwind` boundary catches the unwind, so the child still exits 0 and the parent's `wait_pidfd` sees `Exited(0)` — the failure is masked and the test reports green (mutation testing surfaced this: a ready-fd arm test passed with the readiness mask zeroed, because the "release on nonzero exit" expectation happened to match the child's error exit). Fix: in the child, wrap the body in `std::panic::catch_unwind(AssertUnwindSafe(f))` and `sys::exit` a sentinel on panic; the parent re-panics on any nonzero status. Never rely on "a panicking child exits nonzero" — a forked harness will catch it.

## Ignoring `close()` errors masks over-counted `SCM_RIGHTS` fds in `recv_fd`
`recv_fd` computes `nfds = (cmsg_len - sizeof(cmsghdr)) / sizeof(i32)` and closes every fd after the first. The `/`→`*` mutant over-counts (1 fd → 16) and `close()`s adjacent stack bytes. With the `close()` return ignored, the invalid fds only yield discarded EBADFs and the first (real) fd is kept either way — no observable difference, unkillable. Propagating the error (`.change_context(RecvFdError::Never)`, per §4.4 discarding errors is forbidden) turns the first EBADF into a returned `Never` and the existing success-path test kills the mutant. Kernel-delivered fds always close cleanly, so the check costs nothing on the happy path.

## `recv_fd` cmsg-type guard `&&`→`||` is equivalent — compare tuples instead
The `else if level == SOL_SOCKET && ctype == SCM_CREDENTIALS` branch is only reached by cmsgs the kernel sends on a Unix socket, and the kernel delivers only `SOL_SOCKET` `SCM_RIGHTS`/`SCM_CREDENTIALS` there: verified empirically that `SO_TIMESTAMP`/`SO_TIMESTAMPNS` append nothing on AF_UNIX streams, sender-side cmsgs of other `SOL_SOCKET` types are rejected with `EINVAL` at `sendmsg`, and cmsgs of other levels are silently dropped, never delivered. The mutant was genuinely unkillable — so, as with disjoint flag `|`→`+`, remove the equivalence source: compare tuples, `(level, ctype) == (SOL_SOCKET, SCM_RIGHTS)`. The `==`→`!=` mutants that replaces it with are killable by the existing `NoFd`/`PidMismatch` tests.

## Test `argv[0]`-keyed behavior (busybox mode) by re-execing `/proc/self/exe`
To exercise a code path keyed on the invocation name — e.g. busybox-style builtin dispatch — re-exec the running binary under a different name without creating a symlink: `builtin openat2 --flags O_RDONLY /proc/self/exe %>%exe; builtin exec_fd %exe <name> [args]`. `exec_fd` sets `argv[0]` to `<name>`, so the re-exec'd process behaves as that builtin. A plain symlink named `<name>` also works and, because `argv[0]` carries the full path, additionally exercises the last-`/` basename split.

## Pre-writing more than 4 KiB into a fresh pipe can deadlock under system pipe-page pressure
`test_read_line_from_fd_multi_chunk` wrote 8192 B + `\n` before any reader existed. Linux shrinks new pipes when the system exhausts `fs.pipe-user-pages-soft` (observed: other processes holding thousands of pipes shrank new ones to 8192 B), so the second write blocked forever in `anon_pipe_write` — a random-environment hang that looked like a mutant-test failure. Rule: a test that pre-fills a pipe must fit within the minimum guaranteed capacity (4096 B), or the writer must run in a forked child that blocks on `write` while the parent reads. The forked-writer form keeps payloads arbitrarily large.

## Positional parameters: the deque is `$1..$N`, and `$0` is a separate parameter
Task #134 replaced the old rule ("`state.positional.get(0)` is `$0`, seeded with `sh`/`fdshell`, and `set --`/`source` replace the whole deque including index 0") — it was internally inconsistent: the deque was used as the `$1..` store *and* as the pseudo-`$0` slot, so `set -- a b c` made `$1` = `b`, and a function call that pushed its name at index 0 leaked it into `$0`, `$#`, `$@` and `$*`. The model now is: `state.positional` holds **only** `$1..$N` (0-based storage, so `get(idx - 1)` is `$idx`), and `$0` lives in `state.program_name`, which `set --`, function calls, `source` and `shift` never touch. The entanglement is the lesson: the old `get(idx)` was bash-correct *only* in CLI mode, where a pseudo-`$0` sat at index 0, so fixing the index map without dropping the function-name push breaks function `$1`, and fixing the push alone turns functions into the off-by-one case — the two must land in one task.
Measured bash facts that fix the seeds (do not "fix" them toward a 0-based deque): in **every** CLI mode bash's `$0` is argv[0] and `set --`/functions/`source` never rebind it, so fdshell's existing 1-based parameter numbering *is* bash's argv semantics — the CLI seeds must not be re-based. `-c` mode is the sharp edge: `bash -c 'echo $0' a b` prints `a`, the **first extra argument** (argv[1]), because bash re-bases the extras to `$0..`; fdshell keeps them as `$1..` and pins `$0` to the invocation name (accepted divergence, README), which is what keeps the `CliArgument(i)` traces and `explain`'s `argv[i]` phrasing stable. Script mode seeds `$0` by popping the path off the deque in `main.rs`, and the pop is keyed on `Origin::File`, so `--fd N` (no path word) must not pop. Superseded claim, explicitly: "keep `set --` and `source` consistent by having both replace the whole deque" is dead — the consistency rule now is "both rebind `$1..` and leave `$0` alone", and a superseded lesson must be rewritten here, not left to contradict the code.

## Non-raw string literals in escape-handling tests pre-interpret the escapes under test
Testing `printf` backslash escapes with `"a\nb"` fed the test *real* control bytes — rustc interpreted the escapes — so only the unknown-escape path (literal backslash-d) ever executed and seven `match` arms in `emit_escape` had zero coverage (mutants unkillable). Use raw strings (`r"\n"`) when the input must contain literal backslash sequences.

## A text-replacement drift counter must be signed when replacements can shrink
`alias_expand` tracked the byte-offset drift between original token spans and the edited line as `usize`, doing `*delta += value.len() - (e - s)`. A value shorter than the word it replaces (or an empty alias) underflows `usize` — a debug panic (SIGABRT) and, in release, a wrapped offset that mispositions every later replacement. The drift is a *net length change* and is legitimately negative, so it must be an `isize`. For the out-of-range/invariant path, cast the offset with `s as usize` and let the `Option`-returning `get(..)` bounds-check it: a negative `isize` wraps to a huge index that `get()` rejects with `None`, which maps to a `Never` — no `try_from`, no panic.

## A match guard that re-peeks the scrutinee is always self-referential
`dollar_subst` matched on `peek.peek()` and guarded the `$_` arm with `!name_continues(peek.peek())` — the guard re-peeks the SAME item (`_` itself), so it always saw a name-continuation and the arm was dead code. Peekable has no two-item lookahead: to branch on "the byte after `_`", consume `_` first (or fold the special case into the shared name path, as done now). Mutation testing found it: guard→false mutants were equivalent because the arm never executed.

## A memfd passed via SCM_RIGHTS shares its file offset with the sender
The child wrote the `$_` word into a memfd (offset left at EOF) and sent the fd to the parent; the parent's first `read` returned 0 bytes — the offset is shared through the open file description, so the reader starts at EOF. The receiver must `lseek(fd, 0, SEEK_SET)` before reading. Any string-via-memfd channel needs the same seek.

## Shadowing a Ref guard does not drop it — scope it in a block
`let state = cell.borrow(); …use…; let mut state = cell.borrow_mut()…` failed at runtime with "cannot borrow mutably – shared borrows are active": the shadowed `Ref` guard lives until the end of the enclosing scope, not until the shadowing binding. Compile-time borrow checking doesn't catch this (separate `Ref`/`RefMut` types), so it survives review and clippy. Wrap the shared-borrow section in a `{ … }` block before the mutable re-borrow (see `run_set_xtrace`).

## Test tty-gated behavior from safe-crate integration tests via `script`
`safe/fdshell` has no `libc` dev-dep (safe crates forbid unsafe), so a pty can't be opened in its tests. To exercise code gated on stdin being a terminal, spawn the binary under util-linux `script -qec <bin> /dev/null` with piped stdin — `script` attaches a pty and forwards the pipe into it, making `isatty(0)` true. util-linux is already a test-environment requirement (the nextest wrapper's `prlimit`), so this costs nothing. The REPL's tty-default `ignoreeof` (`|=`→`&=` mutant) was unkillable by any piped-stdin test — only the `script`-wrapped test reaches that branch.

## `cargo mutants` runs only the mutated package's own tests unless `--test-workspace true`
By default cargo-mutants runs only the test targets of the package being mutated. This repo's `unsafe/sys` wrappers are exercised mostly through `safe/fdshell` / `safe/builtins` tests, so a mutation in a sys function no `unsafe/sys/tests/` test covers is reported MISSED even when an fdshell test would kill it — and the fdshell test binary is not rebuilt for a sys mutation (symptom: the sys mutant's line shows `0s test` while an fdshell-only mutant shows `~2s test`). Two fixes: pass `--test-workspace true` (slower — every mutant runs the whole ~1800-test suite), or give each sys wrapper its own integration test in `unsafe/sys/tests/` (preferred — keeps per-crate runs fast and tests the wrapper at its own layer). The latter is the convention; e.g. `unsafe/sys/tests/{access,tty,pipe}.rs` cover the `test` builtin's sys side. When a tty is an *fd var* rather than stdin, `sys::pty::openpty()` (returns CLOEXEC `LocalFd`s) is cleaner than the `script` wrapper above: register the slave as an fd var and drive it through the normal `%var` path.

## `cargo clippy` (no `--all-targets`) skips the lib test target; CI lints it
Plain `cargo clippy -- -D warnings` does not lint `#[cfg(test)]` code, so a `suspicious_open_options` / `needless_borrows_for_generic_args` in a `tests.rs` passes locally but fails `nix flake check --build-all` (which runs `cargo clippy --all-targets`). Lint test files like source: run `cargo clippy --all-targets -- -D warnings`. `open`/`openat2` take `P: AsRef<CStr>`, so pass `cstr(&p)` by value — `&cstr(&p)` trips `needless_borrows_for_generic_args`.

<!-- Trimmed — covered by STYLE.md §2-7:
- "or" in Display → variants too coarse (§4.7)
- Never add #[allow(clippy::...)] in production (§4.9, §7.1)
- From impls discarding source error (§4.3, §4.5)
- Report<T> requires T: Error + Send + Sync + 'static (§4.6)
- displaydoc doc comments only, no #[displaydoc("...")] (§4.6, §4.13)
- ResultExt required for .change_context() (§4.5)
- Variants differing by flag name → parameter (§4.2)
- Do not use .map_err(|_| ...) (§4.4)
- Do not use unreachable!() (§4.10)
- Use ensure!() / bail!() (§4.4, §4.5)
-->

## Redirect source fds must be allocated above the command's highest target
`resolve_path`/`resolve_dup` cloned sources with `F_DUPFD_CLOEXEC(0)` (lowest free fd), which can land exactly on a redirect target. The `dup2(local, target)` then no-ops and the local's `Drop` closes the target fd — silent truncation (data to fd N is lost, targets closed, multi-redirect interleave clobbers the pre-opened `open_redirect_files` temp). Fix: compute `min_fd = max(export_to)` over the whole redirect list + 1, and allocate/clone open fds (`open.rs` re-home, `resolve.rs` path/dup/var arms) strictly above it. Empirical note: the same hazard in `pipeline/child.rs` pipe clones does NOT reproduce — children inherit the low pipe-fd numbers, so a clone never lands on a file target.

## `readlinkat` empty path on a non-link fd is ENOENT, not EINVAL
The path form of `readlinkat` fails with `EINVAL` on a non-symlink, but the fd form (empty path behind an open handle) fails with `ENOENT` — so `readlink %fd` on a regular file exits 2, not 22. Don't assume both forms share an errno; assert each against real kernel behavior (C repro: `open("f", O_RDWR); readlinkat(fd, "", buf)` → ENOENT).

## Tests in one binary share the process: pid-only temp paths collide
nextest runs each test binary in its own process, but tests *within* a binary run on parallel threads sharing one pid — a scratch dir named `fdshell-foo-{pid}` is shared by every test in that binary, so one test's `Drop` can remove another test's dir mid-run (nondeterministic ENOENTs). Name per-test scratch with pid + an atomic counter (see `scratch()` in `unsafe/sys/tests/statx.rs`).

## An over-counting `remaining` feeding a capped `copy_file_range` total is equivalent
`remaining` reports bytes left in `fd` for `copy_file_range %in %out` with no `COUNT`, and its only caller sets `require_exact` from `cfg.count.is_some()` — so when `remaining` runs, `require_exact` is `false` and its value becomes the `total` passed to `copy_file_range(in_fd, out_fd, total)` in `copy_bytes`. `copy_file_range` caps the copy at the bytes actually available in `in_fd` (`end - start`) and the space left in `out_fd`; a regular destination grows freely, so the in-side is the only limiter. The mutant `end - start -> end + start` can only *over*-count (the offset is ≥ 0, so `end + start >= end - start`), and the over-count is capped away — the copied byte count, the only observable result, is identical. It's unkillable by construction: an over-count feeds a capped copy. Don't add a seeking test to catch it; report a size to a `copy_file_range` total that the copy will clamp, and the entry is an accepted equivalent mutant.

## `memfd_create` has no NULL/anonymous special case: a NULL name faults with EFAULT
`memfd_create(2)` always copies the name from userspace (`strncpy_from_user`) — there is no NULL/"anonymous" form, so a NULL name (what the sys wrapper passed for `Option::<&CStr>::None`) returns EFAULT (errno 14) in *any* environment. "Anonymous" is the empty string: `memfd_create("")` succeeds and the fd shows up in `/proc/<pid>/maps` as `memfd:`. The EFAULT was misdiagnosed as an environmental block ("`memfd_create` is blocked in this dev container"), which is why some tests (e.g. the copy_file_range exec tests) were written with regular files from openat2 to dodge memfd — the designs were fine, the stated rationale was wrong. Two takeaways: when a syscall that takes a string argument fails with EFAULT, suspect a bad pointer, not the environment; and cargo-mutants runs fine in the dev container — its "cargo test failed in an unmutated tree" abort was caused by the failing memfd tests, not the container, so once the baseline passes no host change is needed.

## Exact-`COUNT` restore: kernel behaviors of `copy_file_range` short copies (C-verified)
`copy_file_range` into a **non-regular destination fails EINVAL** — the kernel refuses pipes *and* `/dev/null`, so a short copy only ever happens on seekable regular destinations. `lseek` on a pipe is **ESPIPE**, on `/dev/null` it succeeds (returns 0); `ftruncate` on `/dev/null` is **EINVAL**, on an O_RDONLY regular file EBADF; `ftruncate` preserves the offset when it is past EOF; a short copy advances both offsets by the copied count. Design rules (in `copy_file_range::restore`): no `ftruncate`-EINVAL tolerance — restore is only reached after a *successful* `copy_file_range`, which means a seekable regular destination, where the truncate cannot fail EINVAL; a tolerant arm would be unreachable dead code, i.e. an unkillable equivalent mutant, so propagate the error instead. Same species as the disjoint-flag `|` entry: remove the equivalence source rather than test around it. The mirror case (skip the offset capture on a non-seekable `%out`) is likewise left unguarded: a capture failure and the `copy_file_range` failure both surface as the same plain `Syscall` context, so the guard would be unobservable.

## An octal scan seeded at the leading `0`: the ±1 start-index mutant is equivalent
In `arith/lex_num.rs`, the octal branch scans digits from `i + 1` (the byte *after* the leading `0`). The `i + 1`→`i * 1` (== `i`) mutant makes the scan consume the leading `0` as the first octal digit; a leading `0` contributes value 0 in base 8 and advances the scan by exactly one, so it ends at the same index with the same value, and the `08` base guard (which checks `body.get(j)`) inspects the same position. No input distinguishes the two. Accepted equivalent mutant (see `lex_number`'s octal branch).

## The shared `digit_outside_base` guard `d >= radix`→`d < radix` is equivalent — the parser still reports `ArithSyntax`
In `arith/lex_num.rs`, the "value too great for base" guard is the shared helper `digit_outside_base(body, j, radix)` (lines 90–94) with **two** call sites: the octal branch (radix 8, line 30 — the `08` case) and `lex_radix` (bases 2–64, line 60 — the `2#102` case). It bails when the byte at `j`, the first byte after the digit run `read_digits` just consumed, is a decimal digit with value `>= radix`. The invariant that makes the guard equivalent: `read_digits` has already consumed every digit with `value < radix`, so the byte at `j` can only be a decimal digit *outside* the base — `d >= radix` is true whenever a digit is present at all, and `>=` ≡ `<` ≡ `false`. The `>=`→`<` mutant and the whole-function `digit_outside_base -> bool with false` mutant are therefore the same species: the guard never fires. With the guard disabled, `08` lexes as `0` followed by a separate `8`, and `2#102` as `2#10` followed by a separate `2` (two adjacent number tokens), which the parser rejects as trailing input with the identical `ResolveError::ArithSyntax` message and the identical exit status — the `08` reasoning extended to bases 2–64. **The equivalence is message-level, not byte-level**, so do not read "identical" as byte-identical: the message and exit status are identical, but the *source location line* in the error chain differs — the original build reports `arithmetic expression has a syntax error` + `╰╴at safe/fdshell/src/arith/lex_num.rs:62:9` (the radix call site) or `lex_num.rs:32:13` (the octal one), the `-> false` build reports the same message + `╰╴at safe/fdshell/src/arith/primary.rs:49:22` (the parser's trailing-input bail). A 3602-case differential corpus (bases 1–69 × digit runs, leading-zero bases, octal/hex/decimal, trailing-digit and adjacency cases, overflow bases) found 20 cases differing in stderr only in that location line (`2#18`, `2#19`, `3#18`…`9#19`, `2#102`, `08`, `09`, `0109`). The tests pin the message and the exit status, so they cannot tell the mutants apart — which is exactly why the entry is the record. **Do not add a test** for these mutants: the convention for this species is to record, not to test around it (`digit_outside_base -> bool with true` is already caught). Accepted equivalent mutants (the single `ArithSyntax` variant makes the lexer/parser distinction unobservable).

## `powi`'s `if e > 0` squaring guard `>`→`>=` is equivalent — the redundant square is never read
In `arith/binop.rs`, `powi` does `e >>= 1` then `if e > 0 { b = b.wrapping_mul(b) }`; the guard skips the final squaring of `b` once `e` reaches 0. Under the `e >= 0` mutant that last squaring still runs, but the loop condition `e > 0` is now false, so the loop exits and `b` is never read again — the extra `b = b.wrapping_mul(b)` has no observable effect (wrapping, so no overflow panic). A refactor that squares unconditionally would remove the source, but the guard is a standard exponentiation micro-optimization, so it stays and this is an accepted equivalent mutant (see `powi`).

## `getdents` on an `O_PATH` fd fails EBADF: list with `O_DIRECTORY`
The glob walk originally opened intermediate directories with `O_PATH + O_DIRECTORY` (no real access needed). Probed empirically: `openat2(cwd, ".", O_PATH|O_DIRECTORY)` succeeds, but `getdents` on that fd returns EBADF — an `O_PATH` open file description cannot be listed in this environment; `O_DIRECTORY` alone lists fine. So every directory the walk *lists* (root, descent, literal intermediates) opens with plain `O_DIRECTORY`. A literal non-directory last component needs no open at all: its existence is probed with `statx(dirfd, name, AT_SYMLINK_NOFOLLOW)` against the already-open parent (a broken symlink matches, like bash — the name exists in the directory).

## The glob star's recorded resume `pi + 1` → `pi * 1` is equivalent — backtracking re-steps the star
In `match_component`, `b'*'` records `star = Some((pi + 1, ni))` (resume past the star). The `pi + 1` → `pi * 1` (== `pi`) mutant records the star's own byte instead; on backtrack the loop re-steps the `*` byte, which re-records `(pi + 1, ni)` and advances `pi` to exactly the state the original extension produced. A star is only ever recorded by the unquoted arm, so the re-step always takes the `b'*'` arm again — the extra iteration is a no-op with no other side effect. Accepted equivalent mutant.

## The glob backtrack guard's `sn < name.len()` `<`→`<=` is equivalent — `ni >= sn` pins both to the name's end
In `match_component`'s single backtrack arm, the invariant `ni >= sn` holds: a star is recorded with `sn = ni`, forward steps only raise `ni`, and an extension sets `ni = sn + 1` = the new `sn`. So `sn == name.len()` forces `ni == name.len()`, where the guard's second conjunct `ni < name.len()` is false — `<` and `<=` skip the arm identically, and the verdict (`ni == name.len() && pat.get(pi).is_none()`) is the same either way. The same invariant is why the guard needs no pattern-exhausted special case: an exhausted pattern with a fully consumed name is a match regardless of star state. Accepted equivalent mutant.

## `dot_blocked`'s `[`-arm `&&`→`||` is equivalent — a literal `[` never matches a leading dot
In the glob matcher's dot rule, the `[` arm blocks a dot-leading name when the pattern's first byte is an unquoted `[` opening a valid class. The `&&`→`||` mutant additionally blocks a *quoted* leading `[` and an *unclosed* leading `[`, but the matcher treats both as the literal byte `[`, which can never equal a leading `.` — the name fails to match either way. Accepted equivalent mutant.

## A heredoc body is an opaque span — every multi-line byte scanner must skip it
A heredoc body (`cmd <<EOF … EOF`) can contain any bytes, including `;`, `&&`, `|`, `#`, quotes, `<<`, and keyword-shaped lines (`fi`, `done`, `}`, `;;`). Every layer that scans multi-line bytes must jump from the heredoc command line past its last delimiter line, or the body corrupts statement splitting (`segment.rs`), cond-list splitting (`cond.rs::next_boundary`), block keyword depth (`comment.rs::scan_block`), and brace depth (`brace.rs::scan_brace_block`) — four call sites, all fed by the one shared helper `scan::heredoc::skip`/`skip_region`. The same helper feeds the tokenizer: `token::tokenize_statement` blanks the `body_regions` spans in a copy before tokenizing, so an *unbalanced* quote in the body cannot raise `UnbalancedQuote` (the body tokens are discarded anyway — `finish_command` breaks at the first `;` token, pipeline stages trim at `;` — but the tokenizer must not *fail* on them). The byte-level operator rule (`scan/heredoc::operator_delims`: word-break before `<<`, not `<<<`, not after `|`, bare form takes the next word) and the token-level rule (`parse/heredoc::operator.rs::is_operator`) must agree — if they diverge, the scanned spans and the spec allocation per command/pipeline stage silently diverge.

## The brace gobbler's `btype` overwrite: comma outranks `..` by construction
The gobbler's top-level comma branch sets `btype = Comma` unconditionally, while the `..` branch sets `btype = Seq` only when `btype` is still `None`. So a group is a sequence only if it contains *no* top-level unquoted comma, and `valid_seqterm` is consulted only for such comma-free groups. `{1..2,3..4}` is a comma group whose elements (`1..2`, `3..4`) are plain literal words — bash's `expand_amble` never re-tries sequence expansion on an element, so the output is `1..2 3..4`, not `1 2 3 4`. A refactor that makes the two branches symmetric (first separator wins, or `..` allowed to overwrite a comma) silently changes that output; pin it with a test.

## A pre-parse text rewrite is re-tokenized: generated `#`/keyword bytes are re-recognized
`alias_expand` and `brace_expand` rewrite the line text before `parse`, and the normal tokenizer runs on the result — so a generated word that starts with `#` at a word start begins a comment (`echo {#a,#b}` runs `echo` with no arguments) and a generated word that is a block keyword in command position is recognized as one (`{if,fi} hi` fails with `missing 'then'`). bash recognizes comments and keywords in the *original* input and keeps the expanded bytes literal for both (`echo {#a,#b}` prints `#a #b`; `{if,fi} hi` runs `if` as a command). Any new pre-parse rewrite inherits this behavior; the README documents the deviation for brace expansion.

## `effective_incr`'s direction/sign `<`→`<=` mutants are equivalent — the equality cases are unobservable
In `brace_expand/seq/mkseq.rs`, the sign-flip guard is `(spec.start < spec.end) == (incr < 0)`. The `start < end`→`start <= end` mutant differs only at `start == end`, where the flip is unobservable: the sequence has exactly one element (`prevn` is 0, so `count` is 1 either way), the walk never applies the step, and even the `i64::MIN` overflow path returns `Ok(None)` whether the flip runs or not. The `incr < 0`→`incr <= 0` mutant is likewise equivalent because `incr` is normalized `0`→`1` on the line above, so it is never zero at the guard. Both are accepted equivalent mutants (pinned by `min_incr_without_flip_is_invalid`); making either killable would require an observable difference that bash's semantics do not provide.

## `literal_indices`' bound/step mutants are equivalent — the pinned tokens can't match the searched patterns
In `parse/case_block.rs`, the clause walk is bounded by `esac_idx = tokens.len() - 1` and `while pos < esac_idx`, and after each clause's `)` it steps with `pos = close + 1` into a `;;`-pair search over `(pos..esac_idx - 1)`. The gate already proved the last token is `esac`, and `close` is pinned to a `)` by the search just above, so every bound/step mutant (`-`→`+`/`/` on the bound, `<`→`<=` on the loop, `+`→`-`/`*` on the step) merely probes one extra index — the `esac` or `)` token itself — which can never match a lone `;`, form a `;;` pair, or close a clause. The returned indices are unchanged. Accepted equivalent mutants; the exact-index tests in `case_block/tests.rs` pin the rest of the walk.

## A pre-parse text rewrite must protect the heredoc *terminating* delimiter line
`tokenize_statement` blanks the heredoc **body** (`body_regions` spans) but not the **terminating delimiter line**, so a `cat <<{a,b} … {a,b}` leaves the final `{a,b}` as an ordinary word token (after the `;` separator the preceding `\n` emits). A pre-parse rewrite that rebuilds the line from tokens (brace expansion) will brace-expand that terminator into `a b`, so the heredoc's delimiter no longer matches its terminator and parsing fails with `UnterminatedHeredoc` — even though the *operator* delimiter `<<{a,b}` was correctly protected. Rule: a line-rebuilding pass must protect every token that is heredoc structure, identified as the token whose start offset equals a `body_regions` end (the terminator line), not just the `<<` operator word. More generally, "opaque body" does not imply "opaque structure": anything the scanner leaves unblanked (the terminator line, separators) still tokenizes and must be treated as non-word by downstream rewrites.

## A harness jobserver's pipes are inherited by test processes: exclude own pipe targets in /proc fd-table assertions
cargo-mutants (and any jobserver-driven harness) passes its jobserver pipe into every spawned test process as an inherited fd. Pipeline stages and other forked children inherit that fd for their whole lifetime, so a `/proc/<pid>/fd` assertion that counts "exactly N pipes" passes under nextest alone but never holds under the harness. Rule: a test that inspects a child's fd table must first snapshot its own `pipe:[…]` targets from `/proc/self/fd` and count only pipes it did *not* open. Second half of the same lesson: `/proc`-based checks need a stable steady state to inspect, so identify the state by the full assertion itself (poll until some child satisfies it) rather than by process position or a sleep — transient states (pre-exec, mid-cleanup) are not inspectable and must simply be re-polled.

## A mutant "caught" by a flaky test is a false kill — re-run the gate honestly after fixing the test
The review gate reported `close_inherited`'s `j == i` skip inversion as caught, but the catch was a false kill from the same racy test that produced the timeout: a flaky test can "kill" a mutant by timing out. After the test was made deterministic, the honest baseline (`cargo mutants` without `--baseline skip`) showed the mutant was genuinely missed — and it is observable: the inversion closes the stage's *own* capture socketpair, so `true | builtin memfd %>%m` fails `EBADF` where the clean build exits 0. Rule: when a gate reports "N caught, 0 missed" on a run that contained flaky tests, distrust the miss list; the fix is a deterministic test that kills the mutant, not a re-skip of the baseline.

## `git checkout <file>` on an intent-to-add file wipes the file
`git add -N` (intent-to-add) records an *empty* blob for the path. `git checkout <file>` restores the worktree from the index, so it replaces the file with empty content — this bit twice in one session. For a file that is only intent-to-add (never committed), git holds no committed copy to restore from; to revert a change, rewrite the known-clean content (or keep a backup copy). Same root as the nix `cleanSource` trap: until the path is actually staged, the file is invisible to nix builds, and a half-staged (intent-to-add) file is the worst state — present in git status, empty in the index.

## Paths longer than PATH_MAX: build by relative descent, clean up by relative ascent
A syscall's *path argument* is capped at `PATH_MAX` (4096), but a *result* path (`getcwd`) can exceed it — so a CWD with a >4 KiB absolute path is only reachable via a relative `mkdir`/`chdir` chain (one component per step). Corollaries for tests: `std::fs::create_dir_all` cannot build such a tree (each cumulative-path `mkdir` fails `ENAMETOOLONG` past the cap), and `remove_dir_all` cannot remove it either — descend level by level, then `chdir("..")` + relative `rmdir` per level, and only then remove the short root. To *spawn* a process at a deep CWD, do the descent in the parent and spawn with `Command::current_dir(".")` — the child inherits the deep CWD, while passing the deep path to spawn fails `ENAMETOOLONG`. See the helpers in `unsafe/sys/tests/env.rs` and `safe/fdshell/tests/pwd.rs` (task #31).

## `Vec::with_capacity(n)` + `resize(n, 0)` trips `slow_vector_initialization`
The two-pass zero-fill is denied by the repo's `clippy -- -D warnings`; the one-pass form is `let mut v = vec![0u8; n];`. In `no_std` crates use the path form `alloc::vec![0u8; n]` (the crate's `extern crate alloc;` makes the macro path work without a `use`).

## Retrying `getcwd` on a non-too-small errno converges: the too-small-guard mutants are equivalent
In `env/getcwd.rs`, four mutants treat a non-too-small errno (e.g. `ENOENT` on a deleted CWD) as "grow and retry": the initial arm's `buffer_too_small(e)` guard → `true`, the loop arm's guard → `true`, `buffer_too_small` → `true`, and its first `==` → `!=`. Each merely adds `getcwd` attempts in unchanged process state, and a `getcwd` retry in unchanged state fails with the same errno — the returned error is byte-identical, only the syscall count differs. No test can kill them; accepted equivalent mutants. The deleted-CWD tests (`unsafe/sys/tests/env.rs`, `safe/fdshell/tests/pwd.rs`) pin the observable outcome (errno 2, silent exit), so a future change that alters the *outcome* rather than the syscall count still fails them.

## `_IOW*` over a uapi struct with a trailing flexible array member must be sized to the header, not a fixed `digest[N]` field
The uapi `struct fsverity_digest` ends in a flexible array member (`__u8 digest[]`), so `sizeof(struct fsverity_digest)` is only the 4-byte `{__u16 algorithm, __u16 size}` header. `libc::_IOWR::<T>('f', 134)` encodes `size_of::<T>()` into the ioctl command number, so `T` must be that 4-byte header. A struct with a fixed `digest: [u8; 64]` field is 68 bytes and yields a *different* command number → the kernel sees an unknown request and the ioctl fails `ENOTTY` (25) with zero diagnostic value. Fixed by a 4-byte `FsverityDigestHead` + `assert!(size_of::<FsverityDigestHead>() == 4)` in `unsafe/sys/src/fsverity.rs`, which encodes the trap as a compile-time check; the runtime buffer is the 68-byte header+digest. General rule: when wrapping a `_IOW*` ioctl whose uapi struct has a trailing flexible array member, the type passed to `_IOW*` is the fixed prefix (header), and the buffer you hand to the kernel may be larger than that header.

## fs-verity uapi was redesigned in kernel ≥6.13; `FS_IOC_MEASURE_VERITY` returns the *file digest* (SHA of the on-disk descriptor), not the raw Merkle root
The pre-6.13 interface (`FS_VERITY_FL` via `FS_IOC_SETFLAGS` + `FS_IOC_GET_VERITY_PROTECT`) is removed in ≥6.13, replaced by three ioctls: `FS_IOC_ENABLE_VERITY` (133), `FS_IOC_MEASURE_VERITY` (134), `FS_IOC_READ_VERITY_METADATA` (135). The new API cannot express the old one and vice versa. `ENABLE` makes the kernel build the Merkle tree itself — on ≥6.13 it is the *only* way to create a verity file (no external `fsverity-utils` needed). `MEASURE` returns `{digest_algorithm, digest_size, digest}` = the **file digest**: the SHA over the on-disk `fsverity_descriptor`, *not* the raw Merkle root (kernel `fs/verity/measure.c`; the docs' "root hash" wording is loose). Expose it as `digest=` and document it as the "file digest". Digest output is exactly `digest_size` bytes (the kernel writeback size), never the 64-byte buffer. Algorithm codes are 1-based (sha256=1, sha512=2, from the kernel's `fsverity_hash_algs` table). The pinned sha256/sha512 constants in `unsafe/sys/tests/fsverity.rs` are interop pins that fail loudly if a future kernel changes descriptor hashing.

## The nix build sandbox's `/tmp` and `/build` are the host btrfs here, so fs-verity e2e tests run (with skip discipline) under `nix flake check`
The dev container's root and scratch are btrfs with `CONFIG_FS_VERITY=y` (kernel 7.2.4); a probe derivation confirmed the nix build sandbox's `/tmp` and `/build` are the *host's* btrfs, so `FS_IOC_ENABLE_VERITY` succeeds in e2e tests even under `nix flake check`. But that is host-dependent: on a host whose scratch is tmpfs or a verity-less fs, `ENABLE` fails `ENOTTY` (25) or `EOPNOTSUPP` (95). Rule (see `safe/fdshell/tests/verity.rs`): fs-verity e2e tests must skip gracefully on *exactly* those two errnos (`eprintln!` + return) and treat any other rc as a hard failure. Complementary in-process tests use a memfd (never verity-capable) so they deterministically exercise the `ENOTTY` syscall-error path without depending on the host fs (`safe/fdshell/src/child/verity/tests.rs`).

## The kernel silently drops empty `SCM_RIGHTS` cmsgs (sendmsg)
Sending a zero-fd `SCM_RIGHTS` cmsg (`cmsg_len = sizeof(struct cmsghdr)`) on an AF_UNIX stream succeeds, but the receiver never sees a rights cmsg — probed with raw `sendmsg`/`recvmsg`: only `SCM_CREDENTIALS` arrives, no `MSG_CTRUNC`, no error. Consequence: attaching an empty control message to a fragment is unobservable. That is why `net/sendmsg.rs`'s `attach_ctrl` condition reads `!ctrl_sent && !fds.is_empty()` rather than `n > 0`: the `>→>=` mutant of `n > 0` differs only in the zero-fd case — precisely the unkillable equivalent — while every operator mutant of the `is_empty` form (dropping either `!`, `&&`→`||`) changes what the first fragment carries and is killed by `test_sendmsg_retries_eagain_until_fully_sent` (the fd must ride exactly one fragment).

## Abstract-socket `addrlen` must be exact: a padded len folds the zero tail into the address
An AF_UNIX abstract address is `{sa_family, [sa_data: u8; 108]}`, and `sa_data` starts with a NUL terminator. `bind` takes `addrlen` in bytes; the kernel copies `addrlen - offsetof(sa_family)` bytes of the name *including the NUL* into the hash key. Passing a padded len (e.g. the whole `sockaddr_un`) folds the NUL *and every zero after it* into the stored name — the socket is then only reachable at `@name\0\0…`, and a connect at the true name fails `ECONNREFUSED` (111) with no error at bind time. The len is `2 + name_bytes` (family + the name including its NUL). Symptom when it goes wrong: bind succeeds, connect to the same string from another process fails `ECONNREFUSED` — verify with a cross-process connect, not a same-process one (the in-process `connect()` in the same address space sees the padded bytes too, so a test that connects via the *same* padded struct would pass either way). The exact length is built in one place, `unsafe/sys/src/net/addr.rs`, and shared by `bind` **and** `connect`; a new caller must take the `(sockaddr, addrlen)` pair from there, never re-derive the length.

## `inet_pton`'s `s_addr` is `from_ne_bytes`, not `from_be_bytes`
`struct in_addr { __be32 s_addr; }` is network-order *as a whole* — but on a little-endian host, reading the four bytes of an IPv4 address into a `u32` with `u32::from_be_bytes` byte-swaps them (`127.0.0.1` → `1.0.0.127`). The bytes of an `in_addr` must be read with `u32::from_ne_bytes` (identity on the wire byte order for a `__be32` field's *bytes*; the host-order `u32` value then encodes the address correctly for the syscall, which compares raw bytes). Symptom: `bind` succeeds, `connect` fails `EADDRNOTAVAIL` (99) — the socket bound to an unusable address. Probed empirically before fixing (C repro: bind both ways, connect to `127.0.0.1`).

## A bound (non-listening) TCP socket refuses connects by design; probe inet binds with UDP
`connect()` to a TCP port that is *bound* but not *listening* gets an immediate RST (`ECONNREFUSED`), even though `bind` succeeded. So an e2e test that proves "the inet address form works" must either `listen` first (which tests `listen`, not just `bind`) or probe connectionlessly: bind `--type dgram` and `send_to` a UDP datagram — if the port is bound the kernel queues the datagram (a timed `recv_from` on the sender just times out, `WouldBlock`); if it is not, the kernel's ICMP port-unreachable surfaces on the sender's next `recv_from` as `ConnectionRefused`. See `safe/fdshell/tests/sockets.rs::bind_inet_loopback`.

## Rust 1.95: `UnixAddr` is `SocketAddr`; abstract/inet constructors moved to `SocketAddrExt`
`std::os::unix::net::UnixAddr` was renamed `std::os::unix::net::SocketAddr`, and its constructors for non-path families moved to `std::os::linux::net::SocketAddrExt` (`from_abstract_name(bytes)`, `from_inet_v4`-style builders). `UnixStream::connect_abstract` does not exist; use `connect_addr(&SocketAddr)`. When a plan or doc names a std socket API, check the *current* rustc (this env: 1.95.0) — the socket module churns per release.

## A FIFO opened `O_RDWR` is a non-blocking self-pipe; create builtins default `--mode` to the umask-masked convention
`open(fifo, O_RDWR)` succeeds with no peer end (verified: C probe — write+read on the same handle round-trips), which is why `mkfifoat` reopens its just-created FIFO with a single `O_RDWR` flag (one bidirectional handle; two separate blocking ends would need non-blocking tricks). The `--mode` rule for the same builtin family: every create builtin ends with a reopen of the object it just made, and the kernel stores the requested mode as `mode & ~umask`, so the default must be the coreutils value — `0777` for `mkdirat` (a directory needs the owner `x` bit for path lookups through the exported fd: `0666 & ~022 = 0644` is a directory you cannot create files in by path) and `0666` for `mkfifoat` and `openat2 --flags O_CREAT`. The builtin must **not** pre-mask the mode: the kernel applies the umask, and the builtin only reads it (`sys::umask::get()`) to *predict* the stored value. A mode whose masked value lacks the bits the builtin's own reopen needs (`r|x` for `mkdirat`, `r|w` for `mkfifoat`) is rejected in parse by `builtins::mode::require_reopenable`: predicting the kernel's `EACCES` (13) beats relaying it, because the relayed version runs after the object is on disk with no fd captured, and a bare errno reads like a permission problem — the guard names `--mode` and `umask` and creates nothing. Pin modes in e2e tests by putting `umask 0o022` as the first command of the script (the shell applies it with `umask(2)`, forked builtins inherit it, and `sys::umask::set` keeps the cache in sync); builtins unit tests run with the cache uninitialized (`get() == 0`, so effective == requested) and assert pure bit boundaries. Never call `sys::umask::set()` in a unit test: its `debug_assert_eq!` fires when the cache was never initialized (nextest runs debug profiles) and it changes the umask for every test running concurrently in the same binary.

## Over-reserving the cmsg control buffer is an accepted equivalent (net sendmsg/recvmsg)
In `unsafe/sys/src/net/cmsg.rs`, the `+`→`*` mutants of `cred_space()` (line 14, both `+`s) and of `MAX_CTRL` (line 18) over-reserve the recv control buffer (e.g. `MAX_CTRL` 320 → 11200). `recvmsg` hands the kernel the *needed* size as `msg_controllen` (computed from `max_fds`, not from `MAX_CTRL`), and the kernel only ever writes as much control data as the message actually carries, so a larger stack buffer is harmless — no test can observe over-allocation; accepted equivalents, recorded rather than chased. The `+`→`-` mutants of the same expressions are different animals: `cred_space` `-` (→16) truncates the cred cmsg in a combined fd+cred receive and is killed by `test_recvmsg_creds_with_fd`; but `MAX_CTRL` `-` (→240 < the 280 a 64-fd rights cmsg needs) is a **silent stack overflow, verified unkillable**: the wrapper still tells the kernel `msg_controllen = 280`, so the kernel writes 280 bytes into the 240-byte array. Stable rustc enables no stack protector, the 40 overflow bytes land inside the wrapper's own frame, and — because the kernel writes the *real* fd values there and the cmsg walker reads exactly those bytes back — every assertion in `test_recvmsg_max_fds` (and the full 409-test sys suite) passes under the mutant, confirmed by cargo-mutants. The test still earns its place: it pins the max-size path against size-math regressions. A sentinel canary after the buffer would turn the overflow into an observable error, but the overflow exists only under the mutant (unmutated `MAX_CTRL` is always at least the `msg_controllen` the wrapper passes), so the residual is documented instead of paying for a canary on every `recvmsg`.

## Slice the word vector, its quote mask, and its quoted-flag vector with the same offset
`CommandLine` keeps `args`, `args_mask` and `args_quoted` as parallel, equal-length vectors: `args[i]` is described by `args_mask[i]` (per-byte quote mask) and `args_quoted[i]` (the word's raw span contained quotes). Any dispatcher that drops leading words — the binary in `exec`/`become`, the `builtin` keyword, the `set`/`--` flag, the `timeout` keyword — must slice **all three** by the same offset. `replacer/external.rs` passed `args.get(1..)` against the *full* `args_mask`, so every argument carried its predecessor's mask and `exec printf "[%s] " x "a b"` word-split both the quoted format and the quoted argument. The defect is silent for single-word commands, which is why it survived. A missed third slice is just as silent — it disables the quoted-empty-word fix (task #76) for one path with no error. Correct sites to copy from: `replacer.rs` (`.get(2..)` on all three), `intercept/set_positional.rs` (`.get(1..)` on all three), `intercept/timeout_cmd/parse.rs` (`.get(2..)` on all three). Rule for the regression test: pin the *offset*, not just the symptom — use words with **different** quoting (one fully quoted, one not), otherwise an off-by-one and an off-by-two slice are indistinguishable.

## An empty quoted region contributes no bytes, so the per-byte mask cannot say "this word was quoted"
The tokenizer stores a word as its unquoted bytes plus a per-byte quote mask parallel to them. A quoted region that expands to nothing — the `""` in `""$(true)` — contributes **no bytes** to either, so the mask of `""$(true)` is all-`false` over `$(true)`, indistinguishable in shape from an unquoted word. `fully_quoted` (an empty or all-`true` mask) therefore cannot serve: `""$(true)` is not fully quoted (the middle is unquoted), and the empty-mask case it does catch (`""` alone) is already one word via the fast path. The information has to come from the raw token span: `end - start > t.len()` ⇔ the word contained quotes *or escapes* (an escape like `"a\"b"` keeps both bytes, so the span is longer too — intended, since bash keeps such a word whole as well). That derivation lives once in `parse/word_quoted.rs` and is carried as a per-word `Vec<bool>` (`CommandLine.args_quoted`, `ForBlock.words_quoted`) alongside `args`/`args_mask`; `substitute_args` and `expand_for_words` use it to turn "zero fields after splitting" into one empty word (bash: `set -- ""$(true); echo $#` → 1), **except** for a fully quoted whole-word `$@`, whose mask is all-`true` and which *is* the positional words, so an empty deque yields none at all (task #219 — see "The quoted-empty rule must not fire on a fully quoted whole-word `$@`"). Do not "fix" the span test into a quote-only test — the escape bytes are load-bearing.

## Token spans are the contract of the pre-parse rewrites: every word boundary must move `start` as well as the word flags
Every branch that ends a word must advance the tokenizer's `start` position in addition to clearing the word flags (`word_reset` in `parse/token/step.rs` clears only flags; `start` is the position half). The `|` arm reset the flags but not `start`, so a word after an *unspaced* `|` inherited the previous word's start (`a|b` → `b` spanned `(0,3)`). Task #83 filed it as "latent, not user-observable" after checking the caret errors (which use the live `pos` counter) and `semi::token_range` (first-token start); the real consumers were the pre-parse rewriters, which slice the line with exactly the token spans and run on every line: `alias_expand` (`command_positions` returns the token after each `|` *with its span* and `expand_at` replaces that byte range — `alias g=…; echo hi|g` rewrote `hi|g` and deleted `hi|`) and `brace_expand::rebuild` (per-token `line[start..end]` + re-emission of the `line[pos..start]` gap — a `start < pos` gap slice is invalid and the span covers already-emitted text, so the prefix was duplicated: `echo hi|b{x,y}` → `echo hi| hi|bx hi|by`). Rule: when auditing a span/index bug, grep for every reader of the tuple element, not just the one the symptom names.

## A `saturating_sub` in a stage-index comparison aliases two different stages
`if j == i.saturating_sub(1)` in `pipeline/child.rs` matched `j == 0` for **both** `i == 0` and `i == 1`, so the first pipeline stage's stdin was wired to its own output pipe and `cat | cat` deadlocked. A saturating clamp is the wrong tool for "the element before index `i`": it collapses the boundary case onto the first element. Write the relation so it cannot hold at the boundary (`j + 1 == i`) — the extra `i > 0` guard that looks clarifying is dead logic, and both of its mutants (`> → >=`, `&& → ||`) are equivalent. A cheap, hang-free regression probe for a miswired fd: have a builtin `statx` (or any non-reading inspector) report `/proc/self/fd/0` as the *first* pipeline stage and assert on `kind=`/`size=` — a read-based probe can only fail by deadlocking.

## A keyword that opens with `((` must be recognized on raw bytes, and the boundary scan must honour paren depth
The `((expr))` arithmetic command cannot be a token-level keyword: the tokenizer splits `)` into a separator token and `|` into a pipe token, so `((1+2))` reaches dispatch as the word `((1+2` and `((x|y))` as a two-stage pipeline — no token rule can recover the expression. It must be detected in `parse::inner_parse` on the *trimmed raw statement bytes* before `tokenize_statement` (a leading `((` + trailing `))` ⇒ the keyword; a leading `((` without a close ⇒ a clean parse error). The second half of the trap is the cond-list splitter (`cond::next_boundary`), which cut on `&&`/`||` *before* any parse and ignored paren depth, so `((a||b))` was sliced into `((a` and `b))`. The `ScanState` already tracked `$( … )` depth; extending `advance` to consume a top-level `((` pair (raising the same `paren_depth`) and gating `next_boundary` on `paren_depth == 0` fixes the keyword **and** two pre-existing breakages (`$((a||b))` and `$(cmd && cmd)` both failed to parse). The same depth counter is what `scan::heredoc::operator_delims` already gates on, so `<<` inside `((1<<2))` is no longer misread as a here-doc operator. A single bare `(` still opens no depth, so ordinary word/paren splitting (`echo (a) && echo b`) is unchanged.

## `next_boundary`'s `run_start` reset guards the heredoc-skip window inside blocks
The `;`/`\n` `run_start` reset in `cond/boundary.rs::next_boundary` is load-bearing **for block text**: block text (`if … fi`) is passed to `run_cond_list` whole (with its condition bodies inside the span), so a `;`-terminated `<<` run in a block condition (`if cat <<X; then`) puts a `;` before the block text's first unquoted newline. Without the reset, the `heredoc::skip` operator window at that newline widens from `then` back to the block start, finds `<<X`, and — the delimiter line `X` being present — the skip swallows the `&&`-bearing body line into one part, so the if-parse fails in the heredoc layout (`missing terminating delimiter line 'X'`) instead of the part splitting at `&&` and failing with `missing 'fi'`. Top-level statement segments are not affected, because their part text is a subslice of the command bytes only — the body regions travel separately as `run_cond_list`'s `bodies` (Task #79) — so `next_boundary` never sees a `;`-terminated `<<` run with a body at top level; the killing case is the block wrapper. Killed by `block_heredoc_cond_run_and_and_in_body_splits_part` in `safe/fdshell/tests/heredoc.rs`; the top-level window semantics the same reset pins are covered by `heredoc_after_semicolon_word_drives_boundary_skip` and `heredoc_attached_operator_before_semicolon_is_not_a_delimiter`.

## The attached-vs-bare delimiter rule is a raw-byte rule, and a pre-parse protection match must allow the line's optional leading bytes
`<<-` (tab stripping) broke two assumptions that both keyed on the *word* the tokenizer saw, not the raw bytes. (1) `parse/heredoc::operator::attached` decided attached-vs-bare from the *unquoted* token text (`t.len() > 2 || end - start > t.len()`), which cannot see the `-` marker buried in the raw span: a bare `<<-` token read its delimiter as `-` (the marker byte) instead of taking the next word, so `<<- WORD` and `<<- -` mis-parsed. The rule must read the raw operator bytes — `scan::heredoc::op::operator_delim` now returns `(strip, delim)` and feeds **both** the byte scanner and the token parser through one `Operator`, so the two levels cannot diverge (the LESSONS.md "byte-level and token-level rule must agree" trap, made un-recreatable). (2) `brace_expand::protect::heredoc_terminator_indices` matched the terminator as the token whose start *equals* a `body_regions` end. A `<<-` terminator line is tab-indented, so its word starts *after* the tabs and the equality match failed — the "protect the terminator line" failure in reverse (the terminator was brace-expanded and the heredoc looked unterminated). The fix relaxes the match to "the first token that follows the region end across leading tabs only" (`terminator_word`); a plain `>=` is wrong, because it would protect a real statement that follows a *blank* terminator line, which must match nothing — so the gap must be tabs-only. General rules: a token-text predicate that decides raw-byte structure (attached vs. bare) must read the raw bytes, not the unquoted word; and an offset-equality match against a line start must be widened to exactly the region the structure permits — here, leading tabs only.

## A received `SCM_RIGHTS` fd is always `CLOEXEC`, so a sender's `MFD_CLOEXEC` / `O_CLOEXEC` cannot be observed at the receiver
`recv_fd` passes `MSG_CMSG_CLOEXEC`, and the sender's copy is closed before the caller regains control, so a capture-based test can only observe *effects* of the other flags (a seal applied, a size set) — never the `CLOEXEC` bit the sender requested. Pin the creation flags with an in-process test that creates the object and checks the invariant (`LocalFd::verify()`, a successful `set_seals`) — the same reason the sys-level `| EFD_CLOEXEC` wrappers need their own `unsafe/sys/tests/` coverage. See `create_flags_yield_a_cloexec_sealable_memfd` (unit) and `memfd_exec_seals_before_exporting_the_handle` (builtins integration).

## `connect` returns once the peer is queued in the listen backlog — it does not wait for the peer's `accept`
For both AF_UNIX and AF_INET stream sockets, `connect()` completes the three-way handshake with the *kernel* (the SYN-ACK comes from the listener's backlog, not from user space), so it returns as soon as the connection is queued — the peer's `accept()` may happen later. This is what lets one fdshell script be both client and server sequentially (`listen @N %>%l; connect @N %>%c; accept %l %>%s` in a single script, no backgrounding — `connect_listen_accept_in_one_script_abstract`). Only a *full* backlog (or an unreachable peer, `ECONNREFUSED`) makes `connect` block/refuse; a backgrounded `accept` is the non-blocking form for the *server* side, not a prerequisite for `connect`.

## FICLONE uapi was redesigned in kernel 7.x: the new `_IOW(0x94, 9, int)` is issued on the *destination* fd, and the old number is removed
The pre-7.x `FICLONE` (`_IOR(0x94, 0x42, int)`) is removed — issuing it on 7.2.4 fails `ENOTTY` (25). The redesigned ioctl is `_IOW(0x94, 9, int)` (0x40049409, `<linux/fs.h>`), issued on the **destination** fd with the **source** fd as the argument; `libc` (0.2.186) exports the new `FICLONE` constant, so the sys wrapper re-exports it and pins the number with a compile-time `assert!(FICLONE == 0x40049409)` (`unsafe/sys/src/ficlone.rs`). Verified constraints (C probes, btrfs): the destination must be open for writing and the source for reading — both `EBADF`s are checked *before* the filesystem, so they are deterministic on any host; a non-empty destination larger than the source is refused `EINVAL` (no shrinking); an empty source is a no-op that leaves the destination size untouched; a memfd source is `EXDEV` (a memfd is its own tmpfs instance, so cross-device on any host). Same species as the fs-verity >= 6.13 entry: the wrapper targets only the new interface (probing the old number is out of scope), and the reflink e2e tests keep the fs-verity skip discipline (skip on exactly `EOPNOTSUPP`/`ENOTTY`, hard-fail on anything else) for non-reflink hosts.

## The heredoc "unterminated" search must start at the logical line end, not the run end
The REPL's "is this `<<` run unterminated?" check (`scan::heredoc::unterminated`) takes a `(run_start, run_end)` and asks whether the delimiter line has arrived. The tempting `run_end` is `off + stmt.len()` (the end of the statement from `scan_segments`), but for a **complete** heredoc the segment's span is the command line *plus* body *plus* the delimiter line, so `off + stmt.len()` lands past the delimiter and the search (starting at the run end) finds nothing, reporting a *complete* heredoc as unterminated (the REPL would read a spurious continuation line). The run-end must be the command line's **own** terminator — the first `;`/`\n` at or after `off` (`repl/complete::cmd_line_end`) — which bounds the *operator* scan only. The delimiter search itself starts at the **logical line** end (the first unquoted newline at or after the run start, `line_end_after`; the buffer end when the line is unfinished): since Task #79 a `;`/`&&`/`||` after the operator does not end the body search — `cat <<EOF ;` is a continuation, not a hard error, because the body is read after the whole logical line — so keying the search on the run end (the `;`) would miss the body and report a complete `cat <<EOF ;\nbody\nEOF` as unterminated.

## A trailing `|` is a hard parse error, not a REPL continuation — the pipeline parser cannot span a newline
The REPL's completeness check treats a trailing top-level `&&`/`||` as "keep reading" (the cond-list splitter, `cond::next_boundary`, splits on them and happily re-joins across a `\n`), but a trailing lone `|` is **not**: the pipeline parser (`parse/pipeline`) requires the next command on the *same* line, so `echo a |\nwc -w` fails `expected command after pipe` even as a plain script. Including `|` in the continuation rule would make the REPL swallow a continuation line and then fail anyway — worse than the immediate error. So `repl/complete::top_level_op` detects only `&&`/`||`; a lone `|` (and a lone `&`, since fdshell has no backgrounding) stays a hard parse error. General rule: a "trailing operator continues" REPL rule is only sound for operators the parser itself re-joins across a newline — verify each operator against the parser, not against bash.

## `is_bare`'s gate on `top_level_op` is unobservable — its mutants are equivalent
In the REPL's trailing check (`repl/trailing.rs`, used by `repl/complete.rs`), `trailing_open` records a `&&`/`||` as the last top-level operator only when `is_bare(&state)` is true (no open quote, single-quote span, backtick, or `$( )` substitution). `is_bare` carries mutants no input can kill: the function's `-> true`, and each `&&`→`||` on its single expression (`!state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0`). They are equivalent because an in-quote / in-span / in-backtick / in-`$( )` `&&`/`||` can never be the *last* top-level operator with a whitespace tail: a balanced quote leaves a non-whitespace closing `"` (or `'`, backtick / `)`) after it, and an unbalanced one makes the buffer incomplete via the `in_quote` / `in_single` / `in_backtick` / `paren_depth` checks independently. The gate is dead weight for every input that reaches it — accepted equivalent mutants, recorded here so future `cargo mutants` runs and reviews do not re-flag them (the plan's "Mutant hygiene" note anticipated exactly this). Task #141 added the `!state.in_single` conjunct: it is the same class (an unterminated `'` is reported by the quote check in `trailing_open`, and a terminated span leaves a non-whitespace `'` tail), so the `in_single` mutants of `is_bare` are accepted as equivalent too.

## A single-quoted literal is stored in the double-quote escape form
Task #141 gives the tokenizer a single-quote state, but the quote **mask** stays `Vec<bool>` — it is read by ~15 consumers (word splitting, globbing, param-op patterns, clobber/redirect detection, `word_quoted`, `args_quoted`, for-list words), and a per-byte quote *kind* would touch every one of them. So a `'…'` span is encoded as the text the **double-quote arms** of the word readers produce: `\` → `\\`, `$` → `\$`, `` ` `` → `` \` ``, every other byte verbatim, all with mask bits `true` (the `` ` `` pair was added by task #216: the substitution pass reads an unescaped backtick as a command substitution, so the literal backtick of a `'…'` span must reach it in the same escape encoding `$` uses). Word text is read by exactly two layers, and both decode that encoding to the literal bytes: `substitute/arg.rs` (quoted arm, POSIX 2.2.2: `\$`→`$`, `\\`→`\`, `` \` ``→`` ` ``, any other `\X` keeps the backslash) and `bytes/fold.rs::fold_mask` (the path for a word that never reaches substitution — a command name, a redirect target, a `builtin`/`hash` name), whose quoted arm #141 aligned with `arg.rs` so one rule decides both. The encoding is **mask-independent**: `\$`/`\\` fold to the same literals under the POSIX 2.2.1 (unquoted) *and* 2.2.2 (quoted) arms, so an assignment value is right with and without the mask — the paths are `run_dispatch/assign_str.rs` and `run_env::expand_one` (mask-carrying since task #217, and pinned mask-independent by `tests/env_assign.rs::single_quoted_value_is_mask_independent`) and the empty-mask `case_exec` (task #269); that is why #141 needs nothing from #217. Consequences to remember when touching a scanner: (1) inside `'…'` there is **no escape pair** (POSIX 2.2.3) — every `in_single` test must precede the `\` arm, or a span's `\` swallows the next byte (`echo 'a\'` → `a\`, `echo 'a\b'` → `a\b`); (2) `$'x'` is read as the POSIX-sh expansion — the `b'$' if peek == SQUOTE` arm pushes `\$` with mask `false`, so the word is `$x`, never a silent expansion of the variable `x` (bash's ANSI-C gives `x`; accepted divergence, task #156); (3) `fold` and `fold_quotes` read quote bytes off raw text and `fold_mask` reads the mask — keep all three in lockstep: they are one state machine (`fold_impl`) with three contracts (task #207 added `fold_quotes`, the raw-text **quote removal** a heredoc delimiter word gets; `fold` keeps the quote bytes and is now used by its pins alone).

## tokei 14 miscounts every line after a `b'"'` byte literal — quote bytes are `bytes::QUOTE`
The LoC gate (`tools/complexity.py`, tokei 14.0.0) parses Rust, and its parser reads a `b'"'` byte literal as the *start of a string*: from that line on, comment and blank lines are swallowed into the "string" and miscounted as code. `parse/token/step.rs` measured 81 (true ~73) and `brace_expand/gobbler.rs` 89 (true ~77) purely from this artifact, which had parked them in the 80–90 band that task #85 clears. The fix is structural, not cosmetic: shared quote bytes live in `crate::bytes` (`pub(crate) const QUOTE: u8 = b'"'`), so the literal exists exactly once, in a file too small for the inflation to matter, and the scanners match on the named constant (`QUOTE => …`, `c == QUOTE`). Do not write `b'"'` (or reach for `0x22`) at scanner use sites: it re-introduces the miscount and silently re-inflates the file's LoC by every comment/blank line after it.

## A POSIX `[[:class:]]` inside a bracket carries its own `]`, so the class must be recognized before the span is closed
A bracket expression's content is delimited by the *first* unquoted `]` (after the special leading-`]`-as-member rule). A POSIX class such as `[[:alpha:]]` contains a `]` *inside* the class, so a naive span finder stops at the class's inner `]` and treats the rest (`alpha:]]`) as trailing members — the class is silently mis-parsed and `[[:alpha:]]a` no longer matches `a`. The span walker (`glob/match/bracket/span.rs`) must detect the `[:name:]` sequence (an unquoted `:` opening a class, closed by `:]`) and skip past its inner `]`, so the bracket's real closing `]` is the one *after* the class. The membership side (`bracket/class.rs::class_contains`) then maps the class name to a byte predicate. Keep the two in lockstep: a class the span walker recognizes but `class_contains` does not (or vice versa) re-opens the divergence. Unknown class names fall through to literal members (bash treats an unrecognized `[:…:]` as the literal bytes), so `class_contains` returns `Option<bool>` — `None` means "not a class, treat as literal" — not `bool`, which would force a guess.

## The class-skip `close + 1` → `close * 1` in `contains` is equivalent — the re-read converges in one iteration
In `bracket.rs::contains`, after skipping a recognized class that does not contain `b`, the loop resumes with `c = bytes[close]` (the byte after the class) and `i = close + 1`, keeping the invariant `c = bytes[i - 1]`. The `close + 1` → `close * 1` mutant resumes with `i = close`, so the loop reprocesses that same byte one extra time with the invariant off by one. The extra iteration cannot change the outcome: an unquoted `]` there has already returned (the class was the last member), a quoted `]` is a literal member in both orders, an unquoted `\` is handled by the escape arm *before* the member check and leaves the loop state (`i`, `start`, `end`, `plain`) identical to the original's next iteration, and a plain member `c` is tested against `b` with `start == end == c` (same verdict the original reaches next) before the loop advances to the identical state. Accepted equivalent mutant.

## `spec_at += 1` → `*=` in `finish_command` is now killed — two heredocs on one command are accepted
In `parse/command_args.rs`, `spec_at` indexes the command's heredoc spec list as the arg loop consumes `<<` operators. The `+=` → `*=` mutant keeps `spec_at` at 0, so every operator after the first reads `specs[0]` instead of its own spec. Before task #105 this was unobservable: the second `<<` on fd 0 was rejected by `insert_redirect` with `duplicate redirect target`, so the command failed identically under both builds. Task #105 made duplicate targets legal (POSIX #2.5 last-wins), so both bodies are kept and the mutant prints the **first** body twice — killed by `test_heredoc_twice_keeps_both_bodies_in_order` (unit, two distinct bodies) and `heredoc_bodies_of_one_command_are_read_in_operator_order` (integration, `wc -c` of the second body = 3 bytes, not 2). The site is `command_args.rs:61:21` in the current tree, and the accepted (2026-09) → killed (2026-10) history of this file's mutants is told in one place by the entry "## `finish_command`'s `spec_at += 1` mutant became killable when duplicate redirect targets became legal; its `args_from - 1` neighbour is killable too" below, which also covers the `args_from - 1` neighbours at lines 24 and 33 — read the two entries as one story, not as competing notes. Rule: a spec walk that consumes a list positionally is observable the moment the list can hold more than one entry; a parse test asserting the **exact list contents** with distinct entries is the cheap kill.

## A heredoc body is a property of the logical line, not of a contiguous span — split the layers and re-attach
A here-doc's body is not part of the command line's bytes: it sits **after the whole logical line** (the first unquoted newline), in the order of the line's `<<` operators, so `cat <<A && cat <<B` reads `A`'s body then `B`'s — and a body may contain the *other* here-doc's delimiter (the delimiter match is byte-exact per line, in operator order). No contiguous span of the script contains both the command and its body in the order the parser expects, so no amount of skipping inside one span works; the pre-#79 rejection (`here-doc: missing terminating delimiter line`) was the honest failure of that assumption. The fix splits the layers: the scanner keeps the bodies as **separate regions** attached to the statement (`Segment::Statement { cmd, off, bodies }`, from `scan::heredoc::line_bodies_for_line` — each region spans the body lines *plus* the delimiter line and its terminating newline, so a reconstructed command parses as a contiguous heredoc). The execution layer **re-attaches** them: `run_cond_list` takes `bodies: &[Vec<u8>]` in global per-line operator order, each part gets its share (`take_bodies`, count = the part's byte-level `<<` operator count), and `run_part` reconstructs `cmd bytes + \n + bodies` before `run_one` — the parse layer never changes. Block conditions travel through the block text instead: the block span extends past `fi`/`done` **only when the first body region starts at or after the closing keyword** (a condition-position heredoc); a body-position heredoc inside the block keeps the old span. `parse::cond_bodies::condition_bodies` extracts the condition's bodies from the block text — token offsets are relative to the *full* block text, so the token-level operator scan must run over the full line, not a condition subslice. Invariant: bodies are allocated in **global per-line operator order** — the byte-level count (`operator_count`) and the token-level layout (`parse::heredoc::operators`) must agree per part, or the allocation silently shifts (the "byte-level and token-level rule must agree" trap, extended from one operator word to whole regions). The block's trailing body data is kept out of block-structure scanning by truncating the token slice at the closing keyword (`closing_keyword_index`, depth-based).

Task #138 extends the same rule to **every** consumer of a line's body stream: each must first subtract the operators that already claimed **that physical line**. Statements do it in `segment/line.rs::flush_line` (`assigned` = the operator count before the first run), a keyword block in `segment/block/keyword.rs::claimed_regions` (the block claims regions `p..p+k`, where `p` is the number of `<<` operators written on the block's line *before* the block and `k` is the count in its command text), and a condition in `parse/cond_bodies.rs` (`skip` = the operators on the condition's own physical line before it). The scanner computes `p` where it is knowable — it has the physical line, `line_start` and `start` — and hands the block a text built from **its own regions** (`cond/part.rs::reconstruct`, the same re-attachment the cond-list parts use), so in **append mode** the span stops at the closing keyword's separator and no longer swallows a neighbouring statement's text. **Extension mode** (the bodies interleaved with the block's own lines) keeps the old forward-only span and carries no regions, which is why every multi-line shape stays byte-identical. Threading `p` into the condition instead — the literal reading of the task — adds a parameter to five shared signatures (`run_one`, `parse`, `dispatch_keyword`, `tokens_to_if`/`tokens_to_loop`/`parse_elifs`) and ~40 call sites for a number meaningful to exactly one kind of text; slicing the region list where it is computed is the same arithmetic with none of it. Skip counts are bounded to the physical line that owns the stream: a pre-block statement on an *earlier* line claims nothing here (`line_start` is always a physical-line start), which is what keeps the comment-deferred flush case inert.

Task #238 pushes the identical rule down to **arm granularity**: `closing_keyword_index` truncates the token slice at the closing keyword, so in a single-line block an arm body's text stops at its last token (`cat <<T`) and its regions sit past it — the multi-line form only worked because its terminator line is an unblanked token *inside* the arm's token range. An arm claims regions exactly the way a condition does (`parse/cond_bodies::claimed_regions` is the extracted arithmetic, `condition_bodies` is its `ShortCStr` view), and `parse/part_text::part_text` re-attaches them to the arm's text at the four arm-body sites (`if`'s `then`, an `elif` arm, `parse_else_body`, the `while`/`until` body). The append/extension discriminator is `regions.first().0 >= token_range(slice).end` — **the same predicate the block scanner applies at block granularity** (`segment/block/keyword.rs:82`), so it must stay one rule at both granularities: a copy per call site is where it starts to drift. Consequences measured against bash 5.3.9, all pinned: an arm's rebuilt text is self-contained, so a body line that is exactly `fi`/`done`/`else`/`elif` is body data at arm granularity too; an unrun loop body never parses, so `while false; do cat <<T; done` + bodies stays silent (rc 0, bash parity); and an arm slice on a line that carries an *earlier* arm's here-doc claims only its own region (`elif cat <<C; then cat <<T; fi`). Two divergences stay measured-and-pinned, not "fixed": an `&&` written in the **appended body text** re-splits the arm's text before the arm parses (`if true; then cat <<T; fi` + `body && x` + `T` → rc 1, bash rc 0 — task #234, the body-position twin of the block-text split above), and a line whose body stream is **incomplete** yields no regions at all, so in `if true; then cat <<T; elif cat <<E; then echo b; fi` + `bodyT` + `T` the missing `E` body leaves the `then` arm with nothing to claim (rc 1; bash prints `bodyT` with a warning about `E`) — the pinned condition-position rule `cond_bodies_of_an_incomplete_line_stream_are_empty` is the same rule at the other granularity, so fixing one means fixing both. Refactoring `cond_bodies.rs` for the shared arithmetic keeps its window+skip pair (see the `line_start_of` entry below) byte-identical: the accepted-equivalent mutant and its killers refer to that pair, not to the extraction's callers.

## Capping the shared line buffer caps the whole multi-line accumulation — and a capped reader must read in chunks
`repl/line.rs::read_line` appends stdin to the REPL's line buffer. Capping it is a single pre-append check (`buf.len() + take.len() > limit` → bail, same off-by-one convention as `cli::load_script`: exactly `limit` bytes allowed, the next byte fails). Because `repl/run.rs` appends the continuation lines of one logical input to the **same** `buf` in its `while !is_complete` loop, capping inside the read helper bounds the *cumulative* multi-line construct too: an attacker cannot accumulate N×cap bytes through an unterminated `if`/heredoc; the worst case is `cap + 1` (the one `buf.push(b'\n')` in `read_continuation` past the cap before the next `read_line` bails). Same bomb class as the unbounded script loaders (#29) and the unbounded command-substitution capture: every byte-accumulating path gets a `limit` parameter and a pre-append check, with the constant passed in by the caller (no hardcoded constants in the helper). Read stdin in **4 KiB chunks with a persistent position** (`LineReader`, owned by `run`, shared by all `read_line` calls): a chunk can end mid-line, and bytes after a `\n` belong to the *next* line — a pipe has no pushback, so byte-at-a-time was the only stateless option, and it costs one syscall per byte (~5 s for a 64 MiB line on a fast host; past nextest's 10 s slow-timeout in the nix sandbox, where the suite must stay green). Testing a capped reader end-to-end: write exactly `cap + 1` bytes in 4 KiB chunks — no 64 MiB allocation in the test process (nextest caps each test binary's VA at 128 MB), and since the child consumes every byte before bailing, the final write can never hit EPIPE after the child exits. The exact-boundary test (a line of exactly `cap` must be accepted) cannot use a command line: **the child inherits the test's 128 MB VA rlimit**, so a 64 MiB line can never finish in the child (line buffer + script copy alone fill the cap — it SIGABRTs in `to_vec`), and a "command not found" report for a 64 MiB name would be 134 MB of stderr. Use a `cap`-byte **comment line** (a no-op once accepted) and assert only the *absence* of the size-limit error — under a `>=`/`==` mutant the child bails on the cap-th byte with the error and the test fails.

## `FS_IOC_READ_VERITY_METADATA` (≥ 6.13): the descriptor is the 256-byte redesigned struct; a single-block tree item is empty; an unsigned signature is `ENODATA`
The ≥ 6.13 on-disk descriptor is *not* the old 20-byte-header-plus-digest format (the one a pre-6.13 plan would pin): it is the fixed 256-byte `struct fsverity_descriptor` — `{u8 version (=1), u8 hash_algorithm (1-based), u8 log_blocksize (log2 block size), u8 salt_size, le32 reserved, le64 data_size, u8 root_hash[64] (only the first `digest_size` bytes meaningful), u8 salt[32], u8 reserved[144]}`. `READ_METADATA` is `pread()`-like: it returns a byte count (`0` past the item's end and for a zero-length read), `EINVAL` for an unknown `metadata_type` or a non-zero `__reserved`, `ENODATA` for a non-verity fd. Two non-obvious item semantics (probed on kernel 7.2 with a C program before pinning): a file with a **single data block** has an *empty* Merkle-tree item (0 bytes — the root hash lives only in the descriptor), and the **signature** item of an *unsigned* verity file fails `ENODATA` (it is not a 0-byte read). Task #93's plan pinned the old 52-byte descriptor layout and a 32-byte tree — both wrong on ≥ 6.13; the tests pin the probed real values instead. Rule: when a plan pins an on-disk kernel format, probe it first; the descriptor layout is uapi but is easy to misremember (two distinct formats have shipped).

## The `CHUNK` `*` → `+` in `dump_read` is equivalent — the hexdump output is chunk-boundary-independent
In `safe/fdshell/src/child/verity/dump_read.rs`, `const CHUNK: usize = 64 * 1024;` sizes the streaming read buffer for `verity %fd --dump`. The `*` → `+` mutant (CHUNK = 1088) is unkillable: every hex row carries a **cumulative absolute** offset (not a per-chunk offset) and rows are 16 bytes, so the rows concatenated for any chunk size ≥ 16 are byte-identical on stdout. The only differences are the syscall count and the stack-array size, neither observable by any test. The chunk size bounds memory only (the whole point of streaming), so it is an accepted equivalent mutant, recorded here so future `cargo mutants` runs and reviews do not re-flag it (task #93, review finding F1).

## Scoped `NAME=value cmd` assigns must apply **after** the child's `substitute_args`, and `restore` must save the first-touch values plus the old `state.ifs`
A leading `NAME=value` word on a command is scoped to that command (POSIX 2.9.1): expanded against the shell, applied to the command's execution environment (exported to external children), never persisted. Three traps, all pinned by `safe/fdshell/tests/env_assign.rs` and `run_env/tests.rs` (task #120):

1. **Apply after substitution, in the forked child.** `child_main` calls `run_env::apply_child` *after* `substitute_args` and `last_arg::send`, before xtrace/dispatch. Applying earlier would let a scoped `IFS` re-split the command's *own* words: `IFS=: echo a:b` must print `a:b` (the words were split with the parent IFS); only the command's execution environment sees `:`. The parent never applies for forked commands — it expands once (`run_env::expand`), applies/restore **only** around the parent-side handlers (functions, intercepts), and passes the expanded `EnvAssigns` to `launch` → `child_main`.
2. **Expansion must not hold a `RefCell` borrow.** `expand_one` substitutes each value word with no borrow held — a `$(…)` in a value *forks* (command substitution runs a child that borrows the same cell; a held `borrow_mut` would panic). `apply`/`restore`/`apply_child` are the only functions that borrow, and only around plain map ops.
3. **`restore` saves the first-touch values, and the old `state.ifs` separately.** `EnvSave` records, per name, the previous `strings` and `exports` values on **first touch only** — `FOO=pre; FOO=1 FOO=2 cd /tmp; echo $FOO` must restore `pre`, not `1`. And `IFS` is special: `set_var` syncs `state.ifs`, but IFS is not seeded in `strings`, so a scoped `IFS=:` would leak into the parent through `state.ifs` on restore (`strings.remove` is a no-op for it). `record` therefore captures `old_ifs: Option<ShortCStr>` when the name is `IFS`, and `restore` writes it back after the entry loop.

Parse-side invariants: a statement of *all* assignment words (up to `;`) stays a bare assignment (`AssignStr` / new `AssignStrs`); a non-assignment word following the assignments makes it a command with a scoped prefix (`parse/envassign.rs::has_command_word` drives the detect fall-through, `prefix_len` + `collect_range` extract the words). `FOO=bar | cat` (all-assignment pipeline stage) is a clean `expected command` parse error — accepted divergence from bash's no-op subshell component, documented in the README's "Shell variables" section.

## The scoped-prefix parse guard `i + 1` is an accepted equivalent mutant — `detect`/`prefix_len` make it unreachable
One defensive bound in the scoped-assignment parse path (task #120) is unkillable by any test and is recorded here so future `cargo mutants` runs and reviews do not re-flag it (review finding F3); its sibling (F4) was accepted on the same reasoning and later **killed** — item 2 records the correction, not an acceptance:

1. `parse/envassign.rs:41:29` — `assign_at`'s "next token is `;`" guard (`let next = tokens.get(i + 1)?; if next.0.eq_bytes(b";") { return None; }`). The `i + 1` → `i * 1` (== `i`) mutant re-checks the *current* token, which the guard one line above already proved is not `;`, so the arm is a no-op and `prefix_len` stops at the same index. It is also unreachable via the production `parse` flow: `detect` claims every all-assignment statement (`AssignStr` / `AssignStrs`) before `parse_command` runs, so the only input that could reach a `;`-adjacent assignment word is a raw `;`-bearing token slice, which the statement splitter never produces.
2. `parse/command_args.rs:33` — `finish_command`'s `collect_range(tokens, args_from - 1)` end bound is **not** an accepted equivalent mutant (the acceptance first recorded here is superseded). "The command word is by construction never an assignment word — `prefix_len` stopped *there* precisely because it is not `NAME=value`" holds only when the keyword prefix is empty (`kw == 0`): on the `command`/`builtin` path `args_from = prefix_len + kw + 1`, so the widened range swallows the command **name** word, which can be assignment-shaped. `-`→`/` and `-`→`+` are killed by `command_keyword_keeps_the_name_word_out_of_the_env_prefix` (`safe/fdshell/tests/backslash.rs`); the full analysis is the entry "## `finish_command`'s `spec_at += 1` mutant became killable when duplicate redirect targets became legal; its `args_from - 1` neighbour is killable too" below.

## Errexit fires on the final part's status, so a skipped `&&` tail must be excluded with a `ran` guard
Bash's errexit rule keys on *which command failed* (only the command following the final `&&`/`||` can trigger it), but the executor only sees the list's resulting `last_status`. In `cond::run_cond_list`, `false && echo x` skips `echo x` and ends with an **empty final part** that never ran, leaving `false`'s failure in `last_status` — an errexit check on the status alone would exit where bash continues (`set -e; false && echo x; echo after` must print `after`). The guard is `!is_empty_part(line, start, i)`: `run_part` returns `Ok(None)` both for a part that ran cleanly and for an empty part, so "ran" must be decided from the raw bytes, not from `run_part`'s result. The same guard is what makes `true || false` (short-circuit returns before the final segment) and `if`/`while` conditions (the `exempt` parameter) behave like bash. Pinned by `errexit_skipped_and_chain_does_not_exit` (unit) and the `set -e; false && …` integration tests.

## Bash's `-c` mode exits 127 on an unbound-variable expansion error; script mode exits 1
`bash -c 'set -u; echo $x'` exits **127** on bash 5.3 (the `-c` path reports the never-executed command as not-found), while the same two lines in a script file exit **1**. fdshell's `-c` mode reports the child's substitution failure and exits **1** (the script-mode status) — the plan pinned 1 after probing; do not "fix" the integration tests toward 127 to match the `-c` quirk.

## A bare redirect operator token takes the next token as its operand; "bare" is a suffix test, not an emptiness test
`cmd > file`, `cmd < file`, `cmd N> file`, `cmd N< file`, `cmd N>> file`, `cmd N<> file` parse as redirections: a token whose suffix from the first `>`/`<` byte is exactly `>`, `>>`, `<`, or `<>` (with an optional all-digit fd prefix) consumes the next token as its path operand (`parse/redirect/bare.rs`), mirroring the bare `<<` delimiter (`scan::heredoc::op::operator_at`, `extra = 1`) and the bare `<<<` here-string (`parse::here_string.rs`, `extra = 1`). The operand guard (no empty/`;`/`|`/`<`/`>`/`&`/`%`-leading word) is the same rule as the bare `<<` delimiter (`invalid_delimiter`) and the bare `<<<` guard — keep the three in lockstep. The `after_op.is_empty()` test does **not** identify the bare form: `>>` and `<>` have a one-byte (`>`) `after_op`, which the attached path's `redirect_op` rejects as `InvalidRedirect` — that is exactly why `cmd >> file` used to be a parse error while `cmd >file` worked. Test bare-ness on the operator *suffix*; the emptiness branch in the attached path is dead logic once the bare branch exists (deleted — a mutant source). A non-numeric prefix (`a> file`) is two arguments, never a redirect.

Task #145 brought the `&>` / `&>>` family into the same rule. Their bare form *is* an emptiness test (`parse/bg_redirect.rs` strips the `&>` prefix first, so `&>` → empty suffix → bare, `&>>` → empty suffix after the `>` → bare append), and `parse/bg_redirect/bare.rs` emits the fd 1 + fd 2 pair with the operand's own quote mask. Two things made the `>` family's version of the bug possible and are worth remembering: a bare-operand site must **consume** its operand (`skip = extra` in `finish_command`), and an operand left in place re-enters the token loop as an argument — that is exactly `cmd &> file` opening `""` and passing `file` to the command. The operand guard is now literally one function, `scan::heredoc::op::invalid_delimiter`, imported by `parse/heredoc/operator.rs`, `parse/redirect/bare.rs` and `parse/bg_redirect/bare.rs`: "keep the guards in lockstep" is structural, not a copy — a byte-identical private duplicate is what drifts (and is a mutant source). One divergence inside the family is deliberate: the bare `>` maps a `/dev/fd/N` operand to a `RedirectDef::dup`, while `&>` keeps it a plain path source, because the attached `&>/dev/fd/N` form has always opened the magic path (the kernel's dup is the same effect) and in-family agreement beats cross-family agreement.

## A keyword that doubles as a builtin needs the *same* disambiguation at every byte-scanning layer, and the rule must key on what the scanner can actually see
`wait` is both the event-case block keyword and (POSIX #6.7) the `wait [pid…]` builtin. The disambiguation must agree at all three layers that see `wait` bytes, or the REPL (which reuses the byte scanners for completeness) diverges from the parser: `segment.rs` (block vs statement segment), `comment/depth_delta.rs` (block depth while hunting for `done` — it only sees its own run, so `line`+`base` are threaded in to look across run ends), and the token parser (`parse/wait_block::is_block`, which gets the `text` to detect the newline and honors the quoted flag). The rule: a `wait` opens a block iff the next word — skipping whitespace (incl. newlines) and whole-line comments — is **on a subsequent line**, or is a **same-line** pattern keyword (`readable`/`writable`/`finished`/`after`); a same-line pid/`$!`/name, a `;` (statement separator), a quoted word, or EOF is the builtin. Two tempting rules fail: a *pattern-keyword-only* rule misclassifies `wait\ndone` / `wait\n sleeping` (existing block tests) as the builtin and `wait $!` as a block (the byte scanner sees the literal `$!`, not the expanded pid — a pid test fails for the same reason); a *not-a-pid* rule misclassifies `wait; echo hi` as a block (the `;` is a statement separator, not a block arm). The newline is the reliable signal that the arms are coming.

## Linux `struct rusage` has no child-time fields; accumulate them from each `waitid` out-parameter
The plan assumed `getrusage(RUSAGE_SELF)` yields the reaped children's times via `ru_cutime`/`ru_cstime` — those fields exist only on BSD. On Linux `struct rusage` has just `ru_utime`/`ru_stime` (self). The kernel does report a child's `rusage` through the `waitid` out-parameter (what bash uses), so fdshell reads it at every reap site (`sys::localfd::wait_pidfd_rusage`) and accumulates it into `ShellState.child_utime`/`child_stime` (`add_child_times`); `times` reports self times from `self_usage()` and children times from the accumulators. Foreground, background, pipeline, cmd-subst, timeout, and `wait`-arm reaps all feed the total.

## The post-block `line_start` `<`→`<=` and the heredoc `*start = i + 1` in the block scanners are accepted equivalent mutants
Two residual mutants in the block-scanner files (task #117 re-work) are unkillable by any test and are recorded here so future `cargo mutants` runs and reviews do not re-flag them:

1. `segment.rs:75` — the post-block `line_start` update `line_start = if i < heredoc::line_end_after(line, start) { line_start } else { i }`, where `start == i` at that point. The `<`→`<=` mutant differs only when `i == line_end_after(line, i)`; `line_end_after` returns the first bare newline at-or-after `i` plus one (strictly `> i`) or `line.len()`, so equality holds only when `i == line.len()` — the block's resume sits exactly at end of input. The remaining scan there is a final flush of an *empty* run buffer (pre-block runs were drained by the `pre_resume` flush, and nothing buffers past end of input), and `flush_line` returns before reading `line_start` when the buffer is empty. Unobservable.
2. `comment.rs:51` ×2 — the heredoc branch of `scan_block`: `i = resume; *start = i + 1;`, where `resume` indexes the newline terminating the delimiter line. The next boundary the loop sees is that same newline at `i`, so the run it measures is `line[*start..i]`: at most the delimiter line's *last byte* under the `i - 1` mutant, empty under `i * 1`. A single byte is never a keyword (the shortest is two chars), `depth_delta` contributes 0, and the `else` arm resets `*start = i + 1` to exactly the original value before any later run is measured. Both unobservable.

## glibc `%a` normalizes through long double; an f64-based renderer cannot reproduce the trailing hex digits
bash's `%a`/`%A` convert the decimal argument to a **long double** (80-bit on x86_64) and print that value's exact binary significand. fdshell is `f64`-based, so it prints the `f64` value's significand. The two agree on the top 15 hex digits for any value both can represent, but they **diverge from the 16th digit on**: `0.1` is `0xc.ccccccccccccccd…p-7` as a long double (16th digit `d`) but `0xc.cccccccccccd000…p-7` as an `f64` (16th digit `0`); `5e-324` is `0x8.18995ce7aa0e1b2p-1077` as a long double but `0x8p-1077` as an `f64` (the `f64` minimum subnormal has no fractional digits at all). This is a fundamental, unavoidable deviation: there is no stable `f128` in Rust, so the long-double significand cannot be computed. The renderer prints the `f64` form, zero-extended to 15 fractional digits (the `f64` significand width); the deviation is documented in the `printf.rs` module doc and pinned by `hex_float_full_significand_is_fifteen_digits` / `hex_float_precision_above_15_zero_extends`. Do not "fix" the trailing digits toward bash — the `f64` value is the honest result.

## bash `strtol`/`strtod` print the parsed prefix on trailing junk, not 0
A numeric conversion with junk after the parsed prefix (`42x`, `1.5x`, `0x1g`) is an **error** (exit status 1, diagnostic to stderr), but the value printed is the **parsed prefix**, not 0: `printf '%d' 42x` → `42`, `printf '%f' 1.5x` → `1.500000`. Only a *nothing-parsed* argument (`abc`) falls back to 0. fdshell mirrors this in all three numeric parsers: `next_int`/`next_uint` return the `IntParse::Trailing(v)` / `UintParse::Trailing(v)` value, and `next_float` returns `p.value` (the prefix) when `p.consumed < a.len()`. The re-work initially left `next_float` at `0.0` on trailing junk even though the integer conversions had already been fixed to print the prefix — an internal inconsistency and a divergence from bash; the float path now prints the prefix too. Pinned by `integer_stops_at_the_first_invalid_byte` (ints) and `float_trailing_garbage_is_an_error` (floats).

## `|` over a high nibble and a low fraction produces equivalent mutants (hex-float significand)
Two `|`→`^` mutants in the `a`/`A` significand code are unkillable because the operands occupy **disjoint** bit ranges: `mantissa` in `fmt_hexfloat.rs` builds the significand as `(1u64 << 52) | frac`, where `frac` is masked to the low 52 bits (bit 52 clear), so `|` ≡ `^`; and `round_frac` in `fmt_hexfloat_prec.rs` builds `c = (h as u128) << 60 | frac60`, where `frac60` is masked to the low 60 bits (bits 60+ clear) and `h << 60` lives in bits 60–63, so again `|` ≡ `^`. No input can set an overlapping bit, so the mutants are accepted equivalents (recorded rather than chased, per the disjoint-flag `|` lesson above).

## `v < 0` width/precision guards: the `v == 0` case is unobservable
Two `<`→`<=` mutants in the `*`-field resolution and integer padding are unkillable because the equality case has no observable effect. In `resolve_field` (`conv.rs`), the width branch `else if v < 0` (a negative `*` width left-justifies) mutates to `v <= 0`; at `v == 0` the mutant left-justifies a **zero** width, and `pad` fills `width.saturating_sub(…)` = 0 bytes either way — no padding, so left-justify is invisible. In `min_digits` (`fmt_int.rs`), `digits.len() < p` mutates to `<= p`; at `digits.len() == p` the mutant inserts `"0".repeat(p - len)` = `"0".repeat(0)` = nothing. Both are accepted equivalent mutants. (The neighbouring *precision* guard `v < 0`→`v <= 0` **is** killable — a `*` precision of 0 is a real precision, not "absent" — pinned by `star_precision_zero_rounds_to_integer`.)

## The base-0 accumulator `limit` `+1`/`*1` and `value > limit`/`>= limit` are accepted equivalent mutants
The overflow *classification* in the base-0 integer parsers is computed by `clamp`, not by the accumulation loop's `limit`; the loop's `limit` only decides *when* to stop reading digits, and the `end` index it leaves behind is consulted **only** for the non-overflow `Exact`/`Trailing` split. So any mutant that moves the break point by one digit is unobservable — it still overflows to the same `clamp`ed bound with `overflow == true` (where `end` is ignored) or, for a non-overflow value, never reaches the limit at all. Recorded so future `cargo mutants` runs do not re-flag them:

1. `shortcstr/int.rs:59` / `shortcstr/int/uint.rs:55` — `limit = (i64::MAX as u128) + 1` / `(u64::MAX as u128) + 1` mutated `+`→`*` (= the bound itself). The break shifts by one digit, but the overflow result is identical (`clamp` returns the same saturated bound, `overflow == true`, `end` unconsulted).
2. `shortcstr/int.rs:69` / `shortcstr/int/uint.rs:65` — `value > limit` mutated to `value >= limit`. Same reasoning: the break shifts, the classification does not.

The `+`→`-` neighbours (a bound-*minus*-one limit stops the loop at the max value and reclassifies an overflow as `Trailing`) **are** killable and are pinned by `base0_int_overflow_at_max_prefix` / `base0_uint_overflow_at_max_prefix`.

## Two near-duplicate read loops diverge silently: one lost its `eof` flag
`read` had two byte-accumulation loops doing the same job — a byte-at-a-time stdin loop in `io.rs` and a 4 KiB-chunked fd loop in `read_from_fd.rs` — and they had drifted: the stdin loop never set the `eof` out-parameter, so `read x < /dev/null` returned status 0 + an empty variable while the identical `read -u 0 x` returned status 1. The divergence was invisible to every test because the suite only exercised newline-terminated stdin, where `eof` is never consulted. Rule: when two loops accumulate the same line with the same termination rules, factor the state machine into one shared accumulator (here `read/line.rs::Line`, driven by both paths through `read_line_from_fd`) so the two sources cannot drift again; keep the source-specific fd handling in the caller. A status out-parameter that only one of two parallel paths sets is a latent divergence — give the shared state a single `end()` that both paths populate.

## `Line::end_eof`'s `esc` reset is unobservable — an equivalent mutant, removed
`read/line.rs` had `end_eof()` (mark end of input) whose only body was `self.esc = false` — resetting the pending-backslash flag after the last byte. `esc` is private and read only inside `feed()`, and no path feeds a byte after `end_eof()` (`read_line_from_fd` breaks on the empty read; `read_line` then only reads `buf` and `end()`), so the reset is unobservable by any test: the `end_eof()` → `()` mutant is equivalent. The fix removes the equivalence source — `end_eof()` and its call site are gone (the empty-read arm is a bare `break`); EOF is already expressed as `!done` → `LineEnd::Eof` via `Line::end()`. A trailing `\` at EOF is dropped by construction (the backslash is consumed into `esc` and no byte follows to store it), not by the reset. Rule: a state reset that runs after the last observable input, on a field no later code reads, is dead weight — delete it rather than test around it (task #112 re-work, review finding F1).

## POSIX #4.1 escapes: the pair must be skipped by every byte-level scanner, and folded at every word surface
Task #109 implements "outside quotes `\X` removes the backslash and `X` loses its meaning". The pair stays in the token text (mask `false`) so byte offsets and the token-text syntax gates (`if`, `)`, `<<`, `#`, keyword prefixes) keep working, and the backslash is dropped where the word's text is turned into what a lookup sees: at substitution, and at every word surface that never goes through substitution. Consequences recorded as rules:

1. Every byte-level scanner that classifies bytes must consume the pair together, or it diverges from the tokenizer: `scan/advance.rs` (a `\<newline>` would end the line, an escaped `"` would toggle the quote state, an escaped `;` would break the word), `paren_scan.rs` (an escaped `)` closes `$( … )`), `brace_expand/gobbler.rs` (an escaped `{` opens a group). The rule is unconditional (inside double quotes too) — the quoting rule is applied by the tokenizer/substitution, not by the scanner.
2. A pair jump must clamp to the slice end: `advance` returns `(i + 2).min(line.len())`. The unclamped jump lands past EOF, so the caller's end-of-line boundary never fires and the final word is dropped (`printf "[%s]" a\` printed nothing).
3. Fold the escape pairs **in the same pass as** the quote removal, and never after it. The heredoc delimiter rule lives in `scan/heredoc/op.rs::Operator::new` → `bytes/fold.rs::fold_quotes` (one pass over the raw word: quote tracking folds every unquoted `\X` pair and drops every `"`/`'` **marker** byte), so the byte scanner and the token parser share one decision. The first attempt folded inside `matches`, which broke the quoted form: `<<"E\OF"` must delimit the literal `E\OF`, and folding after the quotes are gone cannot tell that the pair was quoted. Task #207 generalised the same reason to the quotes: stripping first cannot tell an escaped quote from a marker (`<<a\"b` delimits the literal `a"b`, quoted = false), so the fold and the strip share one state machine. `Operator` lost its lifetime (`delim: Vec<u8>`, `Clone`) for the same reason.
4. `printf %s\n` is not a newline emitter: POSIX #4.1 folds `\n` to `n`, so bash prints `n` and fdshell now agrees. Integration tests that used `printf %s\n` to print one line per argument were pinning the old fdshell deviation; they now use the quoted `printf "%s\n"` form, which is what bash needs for the same result.
5. **The escape pair must be folded at every word surface, not only at substitution.** bash folds the pair in *every* word at tokenization, so any surface that consumes raw word text and never substitutes it sees the folded text: `e\cho hi` runs `echo`, `echo hi > a\*b` writes `a*b`, `for x in a\ b` binds one word, `exec e\cho hi` replaces the shell with `echo`, `timeout 1 e\cho hi` runs `echo`, `hash a\*` finds the `PATH` file `a*`. The tool is `bytes::fold::fold_mask(bytes, mask)` / `fold_word(word, mask)` — the raw `fold` cannot be used on word text, because the tokenizer strips the `"` bytes from it, so the quote state must come from the word's mask. Surfaces folded (each folds the word *and* its aligned mask): `command_word::expand` (word 0 — task #268 moved the command word from a fold surface to a **substitution** surface, so its pairs fold inside `substitute::substitute_args`; the one expanded word feeds the command-name glob, `exec::resolve_path`, builtin/`command`/`builtin`-keyword dispatch, `intercept` dispatch, the function lookup, `set --`, `prehash`, xtrace and `argv[0]`; see the section below), `redirect/open.rs::redirect_target`, `replacer/external.rs` + `replacer.rs` (the `exec`/`become` command word and the `builtin` keyword's name, whose words 0 `command_word::fields` splits and globs), and `intercept/hash_cmd.rs` + `hash_cmd/pin.rs` (`hash`'s name word, its pin path, and the `hash -r` name list — the table is keyed by the *folded* name, and word 0's expansion folds the same text for a word with no expansion trigger, so `prehash` and `hash a\*` share the key). Surfaces deliberately NOT folded: the arith lexers (`$((1\+2))` stays a syntax error, as in bash) and a backtick command-substitution body (its bytes are command text, never a word surface — task #216 now expands the body through the argument pipeline, but the body itself reaches the child verbatim) — fold only the surfaces that consume word text, and never before a check that keys on the raw bytes. The for-list itself stopped being a fold surface in task #131: it delegates its words to `substitute/arg.rs`, which folds the pair in its own `\\` arms, so `for x in a\ b` binds one word through the substitution pipeline (`tests/backslash.rs::for_list_words_fold_escape_pairs`). A folded word must carry mask `true` on the escaped byte, otherwise the folded `*` becomes a pattern again. The fold's `ShortCStrError` is a NUL-free invariant, so it chains onto the domain's `Never` variant (never `ExecFailed`). Pin every surface with a bash-parity test (see `tests/backslash.rs`); the accepted divergences of the same design are the alias name lookup, the `builtin`/`command`/`if` keyword recognition, and the identifier grammar — all three run on the raw token text, so an escaped keyword word is a command name (`b\uiltin echo hi` looks up `builtin`, rc 1, where bash prints `hi`), pinned by `alias_name_keeps_the_escape_pair` and `keyword_recognition_runs_on_the_raw_token`.

## Word 0 is a substitution surface: expand it once, in the parent, and gate on raw text
Task #268 makes the command-name position expand like an argument: `$(echo echo) hi` runs `echo hi`, and the same for backticks, `$(( … ))`, `$name`, `%fd`/`%CWD` and a leading `~`. The design rules, each one a trap:

1. **Expand at the earliest point that covers every dispatch path, not at each lookup surface.** `run/cmd.rs::run_cmd` expands word 0 before the function lookup, `intercept` dispatch, the builtin keywords and the `PATH` lookup, so all of them see one answer. Expanding in `child/external.rs` (the old word-0 glob site) left `command`/`builtin`/`exec`/`become`/`timeout` and every pipeline stage a literal name, and each surface had to re-implement the split. `command_word::expand` returns `Cow<CommandLine>`, and a word with no trigger, no escape pair and no unquoted pattern is returned **borrowed**, so the common command line pays nothing for the pass.
2. **The gate is a raw-text test, never a mask test.** bash's double quotes never suppress command substitution, so `"$(echo echo)" hi` runs `echo hi` — and that word's mask is `true` on every byte, `$(` included. A mask-gated gate (`trigger byte && !mask`) keeps the bash-parity case literal. `fields::has_trigger` reads bytes only; the fast path (`fields::passes_through`) is "no trigger byte, no `\`, no *unquoted* pattern byte".
3. **Never fold the word before the substitution pass.** A `'…'` span reaches word 0 in the single-quote encoding (`\$`, `` \` ``, `\\`), and the substitution pass is what decodes it; folding first turns `'\$HOME'` into `$HOME` and the pass then expands a variable the user quoted away (the same reason the heredoc delimiter folds in the quote-removal pass, above). Word 0's fold is now the fold inside `substitute_args` — the "fold at every surface" rule keeps holding, the surface list only moved.
4. **Glob exactly once, inside the expansion.** The child globbed word 0 itself and split it (`child/external.rs::split_command`); with the parent doing it, the child must not, or `$(echo ./e)*` globs twice and the second pass globs the already-expanded `./ea`. The child's word-0 glob and split are gone: it resolves the field the parent produced.
5. **The empty-expansion rule needs the quoted *span* flag, not the mask.** "An unquoted empty expansion is not a word, so the next field becomes the command" is a property of the *word*, and `command_mask` cannot say it — `"$(true)"` has every bit set and is a word whose name is empty. `CommandLine::command_quoted` (from `word_quoted::word_quoted`) is the flag; with it set the field list gets exactly one (empty) field, the quoted-empty rule shared with "For-list words".
6. **Fields 1.. of a multi-field expansion must reach the child unexpanded.** They are re-injected as argument words, and a `$`/`` ` `` byte in the child's argv is *its* trigger, so they arrive in the single-quote encoding with every mask bit `true` (`command_word/args.rs::quote_encode`): `$(echo '$HOME') x` looks up the literal name `$HOME`. Without the encoding the child expands it.
7. **Measure against `bash -c` before pinning, and pin the report, not only the rc.** The empty command name and an unmatched literal pattern are fdshell's missing-command error at rc 1 where bash prints `: command not found` at rc 127; a pin that only asserts `rc != 0` passes for the wrong reason when the lookup is skipped entirely (an empty name, a name consumed from the next field, a fold that produced the wrong text). The quoted word-0 name `""` is asserted with `err.contains("\"\"")` for that reason.
8. A scoped `NAME=value` prefix is applied **around** the word-0 expansion (the command name sees `E=echo; $(echo $E)`) and restored before the launch, because the child applies the prefix itself after substituting its args. The visible corner: `IFS=: $(printf "echo:hi") zz` splits word 0 on the scoped `IFS` (fdshell runs `echo hi zz`), where bash splits with the shell's `IFS` and reports the name `echo:hi`. Documented in README, pinned by name in `safe/fdshell/tests/cmd_subst.rs::word0_scoped_ifs_divergence` (which also pins the bash-parity form `IFS=:; $(printf "echo:hi") zz` — the same split when `IFS` is set as its own statement), not by accident.


## Four mutants of `scan/heredoc/ops.rs` are equivalent, and one neighbour of `lines.rs` is (accepted, do not chase)
The `<<` operator scan walks the run with `advance`-driven bounds, and four of its mutants are unobservable, verified by analysis and by `cargo mutants` runs (the loop-bound three are on lines untouched by task #109). Recorded so future mutant runs do not re-flag them. Verify them with a **focused** `cargo mutants -f safe/fdshell/src/scan/heredoc/ops.rs --iterate` run: this file's build takes ~40 s per mutant, so a big `-j6` sweep over all changed files scores them as *caught* (the per-mutant timeout expires before the mutant is tested) — the sweep's count is not the truth for this file.

1. `ops.rs:24:13` `while i < to` → `i <= to`, and `while i < next` → `i <= next` (line 42 when task #109 recorded it, measured at 43:21 on task #141's tree, 24:13/48:21 on task #185's tree — the first coordinate moved off 19 when task #185's helper and doc comment pushed the loop down) — the extra iteration processes one word-break byte (`\n`, `;`, `|`, space, or EOF) that `advance` classifies identically, so `seen_token`/`word_start` and the operator list are unchanged (task #185 renamed the flag and measured the extra step at `i == to` on `line.get(i) == None`: no token byte, so the flag cannot change).
2. `ops.rs:77:13` `while k > 0` → `k >= 0` (the `precedes_pipe` walk back to the `|`, now line 85 after task #103's `clobber_pipe` call, 103:13 after task #185's helper) — the extra step reads `line.get(0)` after `k` has already been decremented to 0. Under the `seen_token` guard a word-start `<<` can never be preceded only by spaces (all bytes before it would have to be whitespace, which makes `seen_token` false), so the loop always returns from the body before `k` reaches 0 and the extra step is unreachable.

3. `ops.rs:27:38` `i = i + p + 1` → `i + p * 1` (the comment skip lands on the comment's `\n` instead of just past it) is **equivalent**. An earlier version of this entry claimed it was killable by `comment_at_run_start_skips_its_text`; that claim no longer matches the tree and task #141 re-measured it: the focused, timeout-safe run reports it MISSED, applying the mutation by hand leaves the whole suite green (4380 tests), and a 17-case comment+heredoc probe (comment at the run start, comment before `<<`, comment inside a run, the `<<'Q'` forms, comment + pipeline) shows no stdout/stderr/rc difference. Analysis: `p` is the offset of the first `\n` from `i`, so the landing byte is always that `\n`, and the loop's next iteration can only fall through to `state.advance(line, i)` (the `#` arm needs `#`, the `<<` arm needs `<`), which consumes the `\n` as one word-break byte and clears `word_active` — the state the comment arm already had, since `!state.word_active` gates it. So `seen_token`, `word_start` and the operator list are identical, at the run start as everywhere else: `advance` *does* get to consume the landing byte (the line is 32:38 on task #185's tree, and the fall-through there reads the landing `\n` as the flag byte — whitespace, so `is_token_byte` rejects it and the argument is unchanged). The `!state.in_single` conjunct task #141 added to the `bare` gate (line 20) is outside the comment skip and changes nothing here. Rule: for an "unobservable because the byte lands on a word break" claim, check that the loop's only fall-through is `advance` for that byte — and prove it by applying the mutation and running the suite, not by reasoning about one position.
4. `scan/heredoc/lines.rs` has **three** loop-bound mutants of this class, all measured MISSED by a focused `cargo mutants -j2 --test-tool nextest -f safe/fdshell/src/bytes/fold.rs -f safe/fdshell/src/scan/heredoc/lines.rs -f safe/fdshell/src/scan/heredoc/op.rs --iterate` run on task #207's tree and each proven equivalent by applying it and running the whole suite green (4532 tests): `56:13` `word_span`'s `while k < to` (line 64 when task #141 recorded it; `delimiter_word`'s removal and the two-quote `word_span` moved it) — the extra step reads `line.get(k)` past `to`, which is `None`, and `k.saturating_add(1).min(to)` clamps to `to`, so a span that runs to the run end ends at the same index; `26:13` `delimiter_span`'s whitespace skip — at `j == to` the body condition reads `line.get(to) == None`, which is not a space/tab, so the extra bound is never entered; `68:13` `first_unquoted_newline`'s `while i < line.len()` — at `i == line.len()` the newline test fails and `advance` returns `i + 1` with the local state left unread (the function returns `None` either way). Recorded with the loop-bound class above so future sweeps do not re-flag them.

## A delimiter word is a word: quote removal strips every quote byte, not one surrounding pair

`<<a'b'c` must delimit `abc`, as bash does (task #207). The delimiter text is built from **raw line bytes**, so it is the one word the shell does not take from the tokenizer — and it had its own private quoting rules: `Operator::new` folded escape pairs with `bytes/fold.rs::fold` (which keeps the quote bytes) and then `scan/heredoc/lines.rs::delimiter_word` stripped **one surrounding pair**, so `<<'x'y'z'`, `<<ab'c'`, `<<""Q"x"`, `<<a"b c"d` kept quote bytes, their terminator lines never matched, and the delimiter's quoting never reached the body's expand/literal flag. All three rules are now decided in `Operator::new` (shared by the byte scanner and the token parser, so the two levels cannot diverge):

1. Quote removal strips **every** `"`/`'` marker byte, in the **same pass** that folds the escape pairs: an escaped quote is data, and a pass that strips first cannot tell `<<a\"b`'s escaped quote from a marker (`<<a\"b` delimits the literal `a"b`, `quoted == false`). The machine is one private `fold_impl(raw, keep_quotes)`; `fold` keeps the bytes, `fold_quotes` applies quote removal and reports `quoted` = a marker byte was seen.
2. `quoted` is *any* quote byte in the word, in any position — the body is literal for `<<a'b'c` exactly as for `<<'abc'`. Do not widen it to `\`: POSIX says "if word is unquoted" and `\` is a quote operator (bash prints `$HOME` literally for `<<\E\O\F`); that is task #263's scope and it re-pins `parse/heredoc/tests.rs::layout_escaped_delimiter_folds`.
3. The byte-level word span must consume **both** quote kinds: `lines.rs::word_span` consumed a `'…'` span whole (added by #141) and a "…" span byte-by-byte, so `<<'a <<b'` and `<<"a <<b"` cut the word at the quoted space and counted the inner `<<b` as a **second** operator — the "byte-level and token-level rules must agree" trap in its pure form. A word rule written at the byte level is only valid for both quote kinds.

Residual (measured, filed as task #266): the delimiter word may now **contain** a `<<` (`<<'a <<b'` is one word whose delimiter is `a <<b`), and the heredoc **terminator line is statement text** — `tokenize_statement` blanks only the body regions, so a terminator line's own `<<b` counts as an operator at both levels (the global per-line body allocation shifts: `here-doc: missing terminating delimiter line 'b'`) and an unbalanced quote byte on it raises `unmatched quote` (`<<a\"b` + body + `a"b`; master fails on that shape identically, so it is pre-existing).


## `finish_command`'s `spec_at += 1` mutant became killable when duplicate redirect targets became legal; its `args_from - 1` neighbour is killable too
`cargo mutants` on `parse/command_args.rs` (task #109 re-work) found two mutants on lines that are not escape-fold code:

1. `command_args.rs:33:73` `collect_range(tokens, args_from - 1)`: the `-`→`/` mutant (range widened by one token, to `args_from`) and the `-`→`+` mutant (to `args_from + 1`) are **killable, not equivalent** — recorded here because the first analysis got it wrong. The claim "the command word is by construction the first *non*-assignment token, which `collect_range` skips" holds only when `kw == 0`. On the `command`/`builtin` keyword path `args_from = prefix_len + kw + 1` with `kw = 1`, so the widened range includes the command **name** word, which can be assignment-shaped: with a `PATH` file named `X=1` that echoes `$X`, the real code prints `[]` for `command X=1` (bash prints `[]`) and both mutants print `[1]`. Pinned by `command_keyword_keeps_the_name_word_out_of_the_env_prefix` (`tests/backslash.rs`), which also runs the `A=2 command X=1` shape. Rule: a range widened by "the word before the args" must be checked on every path that computes `args_from`, the keyword path included. Task #146, which #101's review opened as a request to *accept* this neighbour as an equivalent mutant, is closed by that correction plus the two pins: the integration test above and the in-process unit pin `test_env_assign_prefix_stops_before_the_keyword_name_word` (`parse/tests.rs`), whose `env_assigns.is_empty()` assertions fail on `-`→`/` and `-`→`+` for `command X=1`, `builtin X=1` and `A=2 command X=1` with no fork and no scratch `PATH` file. A focused run on the 2026-10-13 tree (`cargo mutants -j4 --test-tool nextest -f safe/fdshell/src/parse/command_args.rs`, 11 mutants) measures every generated mutant of this file **caught**, the one exception being `command_args.rs:23:5` (`replace finish_command -> Ok(Default::default())`), which is Unviable — so the file has no accepted equivalent mutant and its `args_from - 1` bound must stay load-bearing for any future refactor (task #151's split).
2. `command_args.rs:61:21` `spec_at += 1` → `spec_at *= 1`: `spec_at` indexes the heredoc body specs of the *next* `<<` operator of the same command. It was accepted-equivalent **only while single-heredoc-per-command-fd stood** (two heredocs on fd 0 were rejected at parse with `duplicate redirect target`, so index 1 was never read). Task #105 removed that rejection, so the mutant reads the first body for the second operator and is killed by `test_heredoc_twice_keeps_both_bodies_in_order` + `heredoc_bodies_of_one_command_are_read_in_operator_order`. Rule: "equivalent because the input that reaches here is rejected" is a *temporary* claim — it must be re-checked when the rejection is removed, and the acceptance note should name the rejection it depends on.

The neighbours of `command_args.rs:24:24` (`tokens.get(args_from - 1)`, the command mask) are **not** equivalent: `-`→`/` and `-`→`+` read a later word's mask, and a quoted command word with an escape pair (`"e\cho" hi`) exposes them — the mask must be word 0's, so the pair stays in the name and the lookup misses. Pinned by `command_name_folds_escape_pairs`.

## `assign_at`'s `tokens.get(i + 1)` → `i * 1` mutant is equivalent — its only caller guarantees a command word
`envassign.rs:41:29` (task #109 re-work sweep) is unobservable. `assign_at` has exactly one caller, `prefix_len`, and `prefix_len` has exactly one caller, `parse/command.rs`, which runs it only after `ensure!(envassign::has_command_word(tokens), ExpectedCommand)` — the statement is guaranteed to contain a non-assignment word before the first `;`. The `next` token guard exists to stop the prefix count on a *trailing* assignment with no command word (`X=1;`), which is precisely the shape `has_command_word` rejects, so the guard is never reached: the run ends at the command word, where `split_once_byte` fails, and `i + 1` and `i * 1` agree. Reading the assignment's own token as `next` passes the `;` guard (the caller's `t.eq_bytes(b";")` arm already returned `None`) and re-splits the same word, so the count is identical for every input the caller accepts. Accepted equivalent mutant. Rule: a lookahead guard whose failing case is excluded by the caller's precondition carries unkillable offset mutants — check the caller, and if the precondition is the whole guarantee, record it; the `has_command_word` test in `parse/tests.rs` pins the precondition.

## A function-call frame must be popped on **every** `try_call` exit path
`local` needs a per-call variable frame (`state/frames.rs`). `function_call::try_call` pushes it right after `swap_positional` and pops it *before* propagating the body's result, exactly as `restore_positional` is called before `result?` (task #108). The three exit paths are: the body fails (a `Report` propagates), the body `return`s (`LoopControl::Return` → converted to `None`), and the body `break`s out of a loop inside the body (`LoopControl::Break` propagates to the caller's loop). A frame left on the stack is a silent corruption: the next `local` in a later call writes into the stale frame, so `end_frame` restores the wrong values. Rule: pair the push/pop around the recursive call and pin every exit path with a test (`failed_body_restores_the_frame`, `return_in_body_restores_the_frame`, `break_out_of_the_body_restores_the_frame` in `function_call/tests.rs`).

## A save/restore frame must carry `strings`, `exports` and `IFS` together
The `local` frame is the `run_env::EnvSave` shape (see the scoped-assignment lesson above), and the same three fields are needed for the same reasons: writing `state.exports` is what makes a `local` visible to children forked *inside* the call (`export E=env; f(){ local E=loc; env | grep ^E=; }; f` → `E=loc`, bash parity), and `IFS` needs its old value saved separately because `set_var` syncs `state.ifs` while `IFS` is not seeded in `strings` — `strings.remove("IFS")` is a no-op, so a `local IFS=:` would leak. The declare form (`local NAME`, no `=`) must leave `exports` untouched: bash keeps the export attribute global, so `export E=env; f(){ local E; env | grep ^E=; }` still shows `E=env`. Restore the recorded `IFS` last, after the entry loop.

## `local`'s name comes from the raw word; its value from `substitute_arg` with the assignment mask
The name is the raw word up to the first `=` (never substituted, so `local '$x'=1` shadows the literal name `$x`), and the value goes through `substitute_arg(value, mask, cache, cell)` + `run_origin::assign_origin` exactly as `run_dispatch/assign_str.rs::set` does — no IFS splitting, no globbing, so `local x=*` stores `*` and `local v2=$v` with `v="a b"` keeps the space. Since task #217 the mask is the **value word's own** quote mask, not `&[]`: `local` reads `CommandLine::args_mask` by index (never `zip`) and slices it at the `=` with `parse::envassign::value_mask`, the same offset the `Assign` of the other two surfaces carries (see "A value substituted with an empty mask folds quoted escape pairs as unquoted"). `substitute_args` would glob `x=*` into a file list. Rule: any new assignment-shaped builtin expands its values with the bare-assignment path, not the command-argument path, and passes the value's own mask.

## fdshell had no single quotes — superseded by task #141; `${NAME+word}` is still unsupported
**Superseded for single quotes by task #141**: a `'…'` span now strips both quote bytes and masks every byte in it (`echo 'a b'` → `a b`, `eval 'local z=1'` runs `eval` with the argument `local z=1`), so integration tests may use single quotes freely — `safe/fdshell/tests/quotes.rs` is the pinning file. The **other half of this lesson is still true**: the `${NAME:+alt}` / `${NAME+alt}` alternate-value forms are not expanded (`echo "[${v+set}]"` prints `[${v+set}]`). Write integration tests with double quotes and `${NAME:-alt}`, which are supported; verify the syntax against fdshell itself before pinning a bash-verified expectation, otherwise the test pins a limitation instead of the feature (the pre-#141 single-quote limitation is exactly what this rule caught).

## Integration tests contribute **no** coverage — the fdshell child exits without flushing gcov data
`nix build .#coverage` counts in-process hits only. An integration test spawns `CARGO_BIN_EXE_fdshell` (instrumented), and that child finishes on `exit_group`, not `exit()`/main return, so the gcov runtime never writes its `.gcda` file; the child's lines read 0 hits in `coverage-report.txt` no matter what the test asserts. The repo's print-path code is therefore covered by **unit** tests in the `<module>/tests.rs` file that call the builtin with a real cell and assert the state it leaves behind (`envfilter/tests.rs::list_prints_rules` is the precedent), while the integration file pins the exact bytes on stdout. Rule: when a new builtin prints, pin the bytes in `safe/fdshell/tests/<name>.rs` *and* exercise the print path from a unit test, otherwise `.#coverage` shows the loop as uncovered (the pre-existing 0-hit loops in `set_list.rs::list_fds`, `set -o` bare listing and `alias_cmd.rs` are this same effect).

## The unset-parameter rule is one rule shared by all four arms
POSIX 2.6.2 makes an unset parameter expand to the empty string, and `set -u` makes it an error. The four parameter arms (`$name`, `${name}`, `${#name}`, `${!name}`) must agree, so the empty + nounset decision lives in one place: `resolve_var_name` is the rule for `$name` and `${name}`, the `${#name}` arm bails on the same `UnboundVariable` and prints `0` otherwise (POSIX 2.7.1: the length of an unset parameter is 0), and the indirect arm reuses `UnboundVariable` with the **whole braced body** as the name, which is what makes the nounset message bash's `!q: unbound variable`. A per-arm literal fallback is the drift source: the #118 nounset commit added the bail to `$name` only, and the braced arms silently kept printing literal text. Rule: put the unset policy in the shared lookup, never in the arm that renders the value. Deleting `literal_braced` is part of the same rule — dead code with no coverage is a coverage hole, so remove the literal path once no arm can reach it.

## A mask-alignment test must bind the variable it probes
`var_name_consumption_keeps_mask_alignment` checks that a consumed name's quoted mask bit does not leak onto the following byte. When an unset name expands to empty, the assertion becomes vacuous: an empty expansion has no bytes, so the trailing byte's bit is the same under any mask bug. Bind the name (`v=W`) so the expansion contributes bytes and the trailing `:` keeps its own bit; pin both the bytes and the mask vector. The same rule applies to any test that asserts a property of the *expansion* — an empty expansion asserts nothing.

## Grep for literal-printing tests is not enough: state-asserting unit tests hide the same dependency
The plan's grep over `tests/` found the three integration tests that printed literal text (`set_opts.rs`, `read.rs`, `last_arg.rs`), and the full suite surfaced a fourth: `src/tests/run.rs::string_assign_unknown_var_preserves_literal` asserts the *state* `x=$nonexistent` leaves behind, so it fails with no visible printing. Rule: after changing a value-producing rule, run the whole suite (`--no-fail-fast`); the state-asserting unit tests are the ones grep cannot see.

## A new bail must not steal an accepted literal divergence: guard the degenerate name before dispatching
`${!}` (an empty indirect name) reached `resolve_indirect` because the `!` dispatch in `brace.rs` ran **before** the `${}` empty-name guard, so the new `InvalidIndirect` bail turned a pinned rc-0 literal divergence into an rc-1 divergence. Rule: when a task adds a bail, walk the dispatch order for each degenerate input (an empty name, an empty body), and keep accepted divergences rc-identical to master. Write the literal guard **once, before** the dispatch, so every arm shares it (`${}` and `${!}` are the same case), and give every accepted divergence both a README bullet and a test pin — a divergence in neither is an undocumented rc change.

## The operator scan can manufacture the empty name, so the degenerate guard runs ahead of it
Task #106 moved the `${}`/`${!}` guard ahead of `split_operator` and still left a nameless message: the scan of `${%x}` returns the empty name (a pattern operator at index 0), so `param_value` bailed `UnboundVariable { var: "" }` → `: unbound variable`, which names nothing (STYLE §4.7) and broke master's `%x: unbound variable`. Rule: a scan that *produces* a name is a new source of degenerate input — guard an empty scan name before the dispatch and resolve the whole braced content as the parameter name, so every nounset message prints a name. Pin the scan result (`scan(c"#x")` is a scan fact, not an expansion) **and** the expansion (`set -u; printf "[%s]" "${%x#y}"` rc 1, nounset-off rc 0 empty).

## Verify bash's real output before pinning a test, not the task text
Task #106's description claimed `${v#a*c}` → `bcabc` and `${v##a*c}` → `bc`; bash 5.3.9 gives `abc` and the empty string. The plan's measured table had one more wrong row (`${!ind%bc}` → `abcab` is really `abca`; its `${v+x#y}→[x#y]` row was correct). Shortest/longest anchored matching is a rule you can compute by hand, so check every claimed example against `bash -c` on this machine before writing the assertion, and pin the measured value. Task #176 added two more instances: its description's `${!v+z}` row described the **bare** `+` (not a colon operator at all — #257's family, and master's output is rc 1 `v+z: invalid indirect expansion`, not `[]` rc 0), and its pin `v=undef; ${!v:-z}` → rc 1 was the **target-unbound** case, which bash answers `z` rc 0 (the rc-1 case is the *name* being unbound: `${!undef:-z}`). A row that mixes up a name with its target reads plausibly and is still wrong.

## Word patterns have no `FNM_PERIOD` rule
`${d#*c}` on `d=.abc` strips the whole value, while the pathname glob with `dotglob` off refuses the leading dot. A word pattern is not a pathname: pass `dotglob = true` into `match_component` from `substitute/param_op/pattern.rs` and pin the leading-dot case, or the shared matcher's glob rule leaks into the expansion.

## The nounset rule is operator-family-dependent, the indirect rule is not
`set -u` bails `${undef#q}` (rc 1, `undef: unbound variable`) but prints `x` for `${undef:-x}`: the colon family carries its own word, so it never needs the parameter, and the pattern family has no fallback. Only that **bail** is family policy — pass it *into* the shared lookup (`resolve/param_value.rs`) as a flag: `param_value` checks `set -u`, `colon_value`/`colon_target` are exempt, and the `!name` resolution (strip the `!`, the two `InvalidIndirect` bails, the nounset message naming the **whole body** `!p`) lives in exactly one place (`indirect_target`). Duplicating the `strip_prefix` per arm is the drift source. Three consequences, measured on bash 5.3.9 (task #176): the exemption is about a **value**, not about the reference — `set -u; v=undef_target; ${!v:-z}` prints `z` at rc 0, while an unresolvable reference is rc 1 in every family (`${!undef:-z}` → `undef: invalid indirect expansion`; `p=""; ${!p:-z}` → `: invalid indirect expansion`), word or no word; `:=` resolves its **write target** through the same rule, so `${!v:=q}` assigns the parameter `v` points at (`x` when `v=x`), never a variable literally named `!v`, and bails before writing when the reference is unresolvable; and the degenerate empty indirect name is *unset* for the colon family (bash `${!:-z}` → `z` rc 0) while the pattern family keeps master's rc-1 bail — the `target_name.is_empty()` guard is colon-local, because a new bail must not steal an accepted literal divergence (task #256 owns the pattern side). Verify every one of these against bash before pinning it.

## The pattern's quote mask is the word's mask slice
The tokenizer strips `"` bytes from the word text, so a pattern byte is a *masked* byte, never a `"` byte, and the mask index is `content_start + name.len() + op.word_offset()` (`word_offset` is 1 for `#`/`%`, 2 for `##`/`%%`). Two consequences to pin: a fully quoted word makes every pattern byte literal (`"${v#a*c}"` does not strip), and — since task #217 — the assignment path expands a value with the value word's **own** mask, so a quoted pattern byte stored in a variable stays literal there (`x="a[b"` with `${x#"["a]}` strips nothing, pinned in `tests/pattern.rs::quoted_pattern_byte_in_an_assignment_value_stays_literal`; the empty mask that used to make the quoted `[` strip was the bug of "A value substituted with an empty mask folds quoted escape pairs as unquoted"). A mask-alignment test must bind the variable it probes — an expansion of an unset name prints empty and asserts nothing (existing rule).

## `printf` emits no trailing newline and `echo` drops empty unquoted words
Pinning an expansion that can be empty needs the assignment form: `r0=${v##a*}; printf "[%s]" "$r0"` keeps the empty result visible as `[]`, while `echo ${v##a*}` drops the word and `printf "[%s]" ${v##a*}` collapses the fields. Write pattern/expansion tests as `defs; rN=<expr>; printf "[%s]" "$rN" ...` and expect no trailing newline.

## A side table that must stay sorted by key and stable within a key needs `partition_point`, not `binary_search_by_key`
`parse/bg_redirect.rs::insert_redirect` keeps a command's redirect list sorted by target fd, and the entries of one fd in command-line order, because POSIX #2.5 applies **every** redirection in the order written: the last one to a fd wins, and the earlier ones still open their files. The old code used `binary_search_by_key(&r.export_to, …)` and read its `Ok(_)` as "duplicate target", bailing. That search returns an arbitrary index inside the equal-key block, so it cannot express "insert after the last equal key" — and the insertion it performs for `Err(i)` is where the sorted invariant lives. The correct form is `let at = redirects.partition_point(|x| x.export_to <= r.export_to); redirects.insert(at, r);` — total (no indexing, §4.11), stable within a key, and it keeps the sorted invariant. Rule: when the semantics is "the last entry for a key wins", the list is a *stable* ordered table; a `HashMap<fd, RedirectDef>` is not equivalent, because `resolve_redirects` zips the pre-opened fds with the list by position, so dropping entries silently mis-assigns opened fds to sources. Predicate mutants to kill: `<=`→`>` (cargo-mutants generates it) and `<=`→`<` (cargo-mutants does **not** generate it — it reverses the order among duplicates, so verify it by hand; `test_duplicate_read_redirects_keep_source_order` and the write/close variants catch it).

## POSIX redirect order is observable at the filesystem: pin the created-empty-file fact
`echo hi >a >b` does not merely write `hi` to `b`; the first redirect **opens and truncates `a`**, so `a` exists and is empty. bash 5.3.9 measures exactly that, and so does fdshell once every entry is kept. A last-wins design that deletes the earlier entry at parse time passes the "b contains hi" test and fails `a`-exists, `cat <missing <f2` (rc 1 — the earlier open aborts the command), `set -o noclobber; echo hi >x >y` (rc 1, `y` absent, because the first target blocks) and `set -o noclobber; echo hi >y >x` (rc 1, `y` created empty). Rule: when a spec says "all of them take effect", test the *side effects of the losers*, not only the winner's output — integration tests must read the files the earlier redirects created (`tests/redirect.rs::duplicate_write_redirect_last_wins_and_first_is_created`, `noclobber_duplicate_write_first_target_blocks`).

## A command's fds must be saved above every redirect target of that command
Saving a target fd with `dup_cloexec` returns the **lowest** free fd, which can
be a hole below a later target; the next `dup2` (e.g. `2>&1`) closes its target
fd and clobbers the saved copy, so the restore resurrects the wrong fd. Fix:
`try_dup_above(max_target + 1)` — the same rule the redirect sources already
follow (`resolve_redirects`' `min_fd`). A closed target saves as `None`.

## The redirect scope restores in reverse apply order, on every exit path
The last redirection to a fd is its winner, so restoring forward leaves the
loser in charge. `Scope::restore` iterates `saved.iter().rev()`, exports the
copy (`Drop` closes it) and closes a `None` target. Like `run_env::restore`, the
restore runs on the success path with `?` and on the handler's error path as
`let _ =` (the handler's report is the actionable error).
Pins, all through `Scope::restore()` and never through a copy of it:
`last_wins_within_one_scope` (fd 1 comes back past the loser),
`reverse_restore_leaves_the_shell_fd_on_its_original_target` and
`post_restore_write_lands_on_the_restored_target` (a write after the restore
must land on the restored target, so a forward restore leaves the loser's file
holding that write).

**The saved copy is a script-nameable fd, so the parent-level restore failure is
constructible.** For target fd 1 the copy lands at **fd 6**
(`fcntl(1, F_DUPFD_CLOEXEC, 2) = 6`; nested scopes land 6 then 7), inside the
script-visible range, where bash's save sits at fd >= 10. A body closes it with
`exec 6>&-`, so the parent's `scope.restore()?` (`run/parent.rs:40`) fails: rc 1,
`redirection failed` + `EBADF`, the two-frame `scope.rs:60` report with **no**
`redirect.rs:48` middle frame. Pins:
`run/parent/tests.rs::body_that_closes_the_saved_copy_fails_the_command` (the
copy fd measured by opening the same scope once) and
`tests/redirect_scope.rs::body_that_closes_the_saved_copy_fails_the_call`.
Know the distinction a fd sweep misses: `exec 4>&-`, `5>&-` and `7..11>&-` also
exit rc 1, but they fail at the `exec`'s **own** close (`become_cmd.rs:41` plus
the middle frame `failed to close redirection target fd N` at `redirect.rs:48`),
not at the restore — rc 1 alone does not identify the restore arm. The shell's
`-c` fd table is `0,1,2,3,8,9`, not "0-2 plus its own CLOEXEC fds". The copy is
script-nameable **today**; the fix that makes it unreachable is task **#184**
(`min_fd = max(max_target + 1, 10)`), which owns the floor and its own
README/LESSONS rewrite.

## A test that hand-copies the implementation pins the copy, not the code
Four `redirect/scope/tests.rs` pins copied `Scope::restore`'s loop by hand
(`saved.iter().rev()` in the test body) and asserted through that copy, so they
never called `Scope::restore()`. Deleting `.rev()` from the implementation — the
exact regression the reverse-order rule exists to prevent — left **all 3760 tests
green**, and `cargo mutants` cannot catch it (it generates no `rev` mutator).
Fix: assert through the real entry point, and make the assertion observable at
the fd level (a `/proc/self/fd/N` readlink plus a post-restore write into a
file), not through the data structure the code walks. Prove an ordering pin by
mutating the order and watching the suite fail; a green suite means the pin is
tautological. The same session committed 30 `dbg` `eprintln!` lines: `cargo
clippy -- -D warnings` does not see them, `cargo nextest run <filter>
--no-capture` does — debug scaffolding never goes into a commit.

## `exec`/`become` must opt out of the redirect scope restore
They replace the process image, so `exec >file` stays permanent. `run/parent.rs`
gates on `is_in_process` (the intercepted set minus the process-replacing
family), so the in-process set and the dispatch table must stay in sync:
`is_intercepted` includes the `quit`/`.` aliases that `INTERCEPTED_COMMANDS`
(`help`) does not list; a dispatch-table word missing from the set is silently
sent to the forked launch path. Pinned by
`intercept/tests.rs::intercepted_commands_are_in_process_except_the_replacers`.

## Closing fd 2 in a test silences the harness
A test that closes fd 2 (e.g. to leave a low fd hole for a scope test) loses its
panic messages: libtest reports FAILED with an empty failure block, and later
`eprintln!`/`println!` diagnostics vanish. Reopen fd 2 (`/dev/null`) before the
remaining assertions.

## A `|` preceded by `>` is the `>|` operator byte, and the byte-level and token-level rules must call ONE predicate

POSIX #2.2 `>|` (clobber: a write that bypasses `noclobber`) makes the tokenizer's
`|` rule load-bearing: a `|` absorbed into a `>`-terminated word is an operator
byte, not a pipeline pipe, and the byte-level heredoc scan must see the same byte
the same way, or the two `<<` counts drift silently. `cat >| <<EOF` counts 1
token-level and 0 byte-level (the scan hits the absorbed `|`), `echo a>|<<EOF` 1
byte-level vs 0 token-level, and a real script breaks observably: `cat "a>"| <<EOF`
then the body then `EOF` reports `missing terminating delimiter` on the divergent
tree, where master and bash read the body (rc 1 / rc 0).

ONE predicate decides both layers: `parse/redirect/clobber.rs::clobber_word`, over
the word's RAW bytes + quote mask — the word's last byte is `>`, unquoted and not
the second byte of an escape pair, and no unquoted `>`/`<` byte comes before it; a
`%`-leading word (the capture operator) keeps its own absorption, where earlier
operator bytes do not disqualify it. The tokenizer passes its word text and mask
(`parse/token_pipe.rs`); the byte scan rebuilds the word from the line with the mask
scanned from the line start (`scan/heredoc/ops.rs` → `clobber_pipe` + `quote_mask`),
so a word that begins with a closing `"` gets the quote state it really has.
Reading the *unquoted* word text, or the byte right before the `|`, is NOT the same
rule: `echo "a>"|b` absorbs the pipe (one word, rc 0) where master and bash give a
pipeline (rc 1, 127); `cat "a>"| <<EOF` measures byte=0 token=1; `cat x\>a>| <<EOF`
measures byte=1 token=0 under the word-text rule — the same trap as "a token-text
predicate that decides raw-byte structure".

Corollaries: a `|` separated by whitespace from a bare `>` stays a pipeline pipe
(`> | f`); `>>`, `>&`, `<>` and `a>b` never absorb (an unquoted operator byte
precedes the last byte); a quoted `>` never terminates a word, and `x\>` never does.

Pins: `parse/token/tests.rs::clobber_pipe_absorbed_into_operator`,
`quoted_word_is_not_terminated_by_the_operator_byte` and
`prefix_word_absorbs_pipe_is_one_word` (`echo x\>a>|b` is one word — the word-break
deviation from bash, recorded in README), the byte/token count agreement list in
`parse/heredoc/tests.rs::operator_count_clobber_forms_agree_at_both_levels` with
`scan/heredoc/ops/tests.rs::clobber_pipe_is_an_operator_byte_not_a_pipeline`, and
the real-script pair `tests/redirect.rs::quoted_operator_word_keeps_the_pipe_and_the_body_free`
/ `quoted_operator_word_does_not_shift_the_next_heredoc_body`. The *run-start* half
of the `cat >| <<EOF` / `>| <<EOF` divergence (the byte scan required a preceding
command word, so a `>|`-leading run counted 0 against the token layer's 1) is closed
by task #185 and pinned by `parse/heredoc/tests.rs::operator_count_leading_operator_forms_agree`
and `scan/heredoc/ops/tests.rs::redirect_operator_leading_run_counts_the_operator`
— see the entry "The byte scan's 'preceding token' test must mirror the token
layer's `i != 0 && prev != |`".

Residual equivalent mutants (accepted, do not chase): `clobber.rs:50:13` `i < e` →
`i <= e` — the extra step appends one mask entry at index `e`, which is outside
every word slice; `clobber.rs:57:28` guard `!in_quote` → `false` — the pair push is
index alignment only (the predicate's escape-pair skip lives in `clobber_word`), so
no reachable word distinguishes it; `clobber.rs:77:13` `s > 0` → `s >= 0` — `usize`
is never negative, and the guard exists so a debug build cannot panic on the
`s - 1` underflow at `s = 0`.

## Model a redirect bypass as a `RedirectDirection` variant, not a flag on the def
`>|` clobber is the direction `Clobber`: `redirect/open.rs` gates the noclobber
check on `matches!(r.direction, RedirectDirection::Write)`, so a direction that
is not `Write` *structurally* cannot be gated, and the bypass needs no change in
`open.rs`. A `clobber: bool` field on `RedirectDef` would have touched the def
struct, `insert_redirect`, the `Scope`, and every `RedirectDef` constructor
(`var`/`dup`/`heredoc`/…), and clearing the `NOCLOBBER` bit during the redirect
would mutate shell state (the `set -o` / `shopt -q` contract, pinned by
`tests/shopt.rs`, and reentrancy). The mutation cost of the design is recorded so
future mutant runs do not re-flag it: `Clobber.open_flags()` shares the
`Self::Write | Self::Clobber` flag arm, so `bare::direction`'s `Write`→`Clobber`
mutant (and `redirect_op`'s `Write`→`Clobber`) is **equivalent when noclobber is
off** — identical flags, identical open path. It is killed only by the noclobber
tests (`shopt.rs::clobber_bypasses_noclobber_on_an_existing_file` sets the option
and measures both the `>|` bypass and the `>` control), so the noclobber half of
the suite is what makes the shared arm observable. `matches!(r.direction, Write)`
→ matching `Clobber` is killable by the same pair.

## Recognize the `{ …; }` group at the byte level, and skip a nested group whole
`segment/group/word.rs` holds one boundary rule (`is_opener_boundary`) used by
the word predicate `opens(word, line, word_end)` (the `keyword_delta` `+1`) and
by the byte predicate `opens_at(line, i)` (the scanner, `cond/boundary`). A word
predicate alone is not enough for the scanner: the first version passed a
hard-coded `b"{"` word to `opens`, so the byte at `i` was never checked and
`local v=1; g` started a "group" at `g` (`function_call::tests::nested_calls_restore_the_inner_frame`
failed with a parse error). `opens_at` checks the byte; `ScanState::at_word_start()`
is the shared four-guard (`!in_quote && !in_backtick && paren_depth == 0 &&
!word_active`) the scanner, `statement_end`, and `cond/boundary` all need.

The group's span is the **statement** it opens (`group_block` → `statement_end`),
so `} > out`, `} && echo b`, `} | cat` stay inside the group and the parse layer
decides the trailing operators. `statement_end` must treat a **second group in
the same statement as opaque** (`skip_group` jumps past its closing brace): with
the plain `;`-based scan, `{ echo a; } && { echo b; }` spanned only up to the
first `;` after the closing brace, cutting the second group, and the cond split
reported `group is missing a closing brace` for `{ echo b`. The `cond/boundary`
group-depth counter (`&&`/`||` only split at `group_depth == 0`) relies on every
group in the statement being a whole span — and the `group_at_run` + `skip_group`
design **enforces** that span: a cond-part group is skipped whole inside the run, so
its inner `;` is never a top-level separator and the counter sees balanced
open/close pairs.

A group is the current shell, not a subshell, so `exec >file` inside the body is
**permanent** (bash agrees): `Scope::restore` undoes the group's own trailing
redirections at the group end, and must not be "fixed" to undo `exec`. Pinned by
`tests/group.rs::exec_in_a_group_is_persistent` (stdout empty, the file holds
`after`). The same fact makes the group's assignment persist
(`group_body_runs_in_the_current_shell`), which is the fork-free proof.

## Count brace depth per exact `{`/`}` word, not per clause position
The POSIX grammar puts `{` in clause position (`;`/`then`/`else`/`do`), so a
depth rule that counted braces only in clause position would drop the `+1` for a
group in a keyword-block body, and `closing_keyword_index`/`comment::depth_delta`
would close `if`/`case`/`function` at the group's `}`. `keyword_delta` gives
`+1` for the `{` word and `-1` for the `}` word with the same boundary set the
scanner uses, so `if { true; }; then echo yes; fi` and
`f() { { echo nested; }; }; f` close at their own keyword. The accepted cost is a
literal brace argument inside a block body: `if true; then echo {; fi` prints `{`
(bash agrees), and `if true; then echo }; fi` reports `"fi" not found` rc 1
because the `}` word eats the `fi` at depth — pinned by
`tests/group.rs::literal_brace_argument_in_a_block_body` with both sides measured
against bash. A clause-position-aware rule is a separate task.

## A group after `&&` is a cond part: skip its span, never flush the run before it
Round 1 flushed the text before a mid-run group opener as a statement and pushed
the group as its own `Segment::Block`. Both halves were wrong. `false && { echo g; }`
ran the group (bash skips it, rc 1): the group left the cond list, so the
short-circuit could not skip it. And the flushed run's offset was the *raw* run
start, not the trimmed part's first byte, so a leading space shifted the
statement slice and cut its tail: inside `$( echo inner && { echo g; } )` the
substitution body starts with a space, the statement became `echo inner &`, and
the parse reported `unexpected character`. The fix: `group_at_run` classifies the
brace (empty run → block; run ends with `&&`/`||` → cond part; else word data),
and a cond part's group is **skipped whole inside the run** (the same `skip_group`
`statement_end` uses), so the body's `;` is never a top-level separator, the run
keeps the group's text, `cond` splits the operator into one part per group, and
`&&`/`||` short-circuit apply to the group. Pinned by
`tests/group.rs::group_part_respects_the_cond_short_circuit` and
`substitution_group_after_operator_keeps_the_run_before_it` (both fail on the
flushing version), and by `segment/group/tests.rs::group_after_and_stays_a_cond_part_not_a_block`.

## A `max(0)` clamp on the closer does NOT make `+=`→`-=` on the opener equivalent
`cond/boundary` counts group depth: the opener is `group_depth += 1`
(**unclamped**), the closer is `group_depth = group_depth.saturating_sub(1).max(0)`
(clamp on the **closer only**). The `+=`→`-=` mutant drives the depth negative;
the next closer clamps `max(0, -n)` back to `0`, so a negative depth is
indistinguishable from `0` **at that closer** — the true and mutated counters agree
on every *single-open* statement. They diverge only on the **counted-open,
counted-open, closer, then operator** sequence: true depth is `+1` at the operator
(no split) while mutated depth is `0` (split), so the mutant splits inside the body
and reports `group is missing a closing brace` rc 1. Kill it with
`tests/group.rs::nested_group_at_a_cond_part_start_keeps_the_body_depth`
(`{ echo a && { echo b; } && echo c; } && echo d` → bash `a b c d`, plus the `||`
variant). The rule: a per-closer clamp keeps the depth's **sign observable through
the clamp**, so the test must cover the nested-open sequence — not just the
`== 0` predicate. Do not assume an unclamped counter is sign-blind.

## A guard duplicated inside the callee is an equivalent mutant
`cond/boundary` skips a heredoc body with `if b == b'\n' && let Some(resume) =
heredoc::skip(line, run_start, i)`. `skip_region` already returns `None` unless
`line[run_end]` is `'\n'` (or the line end), so the `b == b'\n'` test adds no
behavior: the `==`→`!=` mutant is unkillable. It is **pre-existing on `master`**
(`cond/boundary.rs:19:18`), not task #97 work — the same mutant is missed in the
`master` mutation run. Delete the outer test when the callee guards the same byte.

## An unclosed brace scan ends at the line length, so `min(len, close + 1)` and `min(len, close)` agree
`scan_brace_block` reports `close = line.len()` when the group never closes, so
in `group_block` the `close + 1`→`close * 1` mutant gives the same `open_end`
(`line.len().min(len)` = `line.len()`), and the suite passes under it: an accepted
equivalent mutant. The `+1` is still required for the closed case (the `+1`→`-1`
mutant is killed by `segment/group/tests.rs::span_ends_past_the_closing_brace`),
so keep the offset and record the `*` survivor rather than chasing it.

## Byte-level definer words must be the tokenizer's tokens
The `function name { … }` definer rule is one implementation (`keywords/function.rs`:
`name_len` + `function_def_name`) shared by the byte-level span scanner
(`brace::scan_function_block`) and the token-level parse dispatch
(`parse::function_block`), so the segment span and the dispatch accept exactly the
same shapes. The byte word split must equal the tokenizer's word split, and the
tokenizer emits `)` as a separator **token** (`(` is a word char): a lone `)` is a
word of its own, not a terminator to skip. Skipping it shifts the following words,
so `function ) f {` would pair `function` with `f` at the byte level while the token
layer pairs `function` with `)` and rejects it — the span would be claimed by text
the parser runs as commands. A definer word `)` is therefore produced by the byte
split and rejected by `name_len` (which rejects `)`, `{`, `}`, whitespace, and any
`(` that is not the word's last byte). The same rule covers a `)` **glued to the
preceding word**: `function)f` splits into `function` + `)`, the rest returned with
a word must start at the `)` (never past it), so `)` is always emitted as the next
word and never consumed. Consuming the glued `)` claimed `function)f g { … }` as a
`g`-named block that the token layer rejects, so `echo hi` was silently dropped
(rc 0, killed by `keywords/function/tests.rs::glued_close_paren_after_the_keyword_is_its_own_word`).
The same alignment makes `function f ( ) {`
work, and keeps `f ( ) {` (POSIX form, split parens) rejected by both layers.

## `cargo mutants` dies with a nutmeg "View mutex" panic when the filesystem is full
Symptom: the run aborts in its first two minutes with
`thread '<unnamed>' panicked at .../nutmeg-*/src/lib.rs: View mutex is not poisoned: PoisonError { .. }`,
and `mutants.out/outcomes.json` stops after ~10 outcomes (`success: false`). The
run's `missed.txt` is then empty, so a crashed run *looks* like "0 missed": the
only honest completion signal is the `N mutants tested in ...m: ... caught, ...
unviable` summary line. cargo-mutants builds every mutant in a fresh copy of the
tree under `$TMPDIR` (default `/tmp`), so a full filesystem makes its child
processes fail to write, one worker thread panics, and the progress view takes
the whole run down (with `-j1` too — parallelism is not the trigger). Rule:
`df -h` the whole filesystem (not just `/tmp`) before a mutant run; if it is at
100%, free the space the crash itself leaves behind (`/tmp/cargo-mutants-*.tmp`
lab dirs, stale `/tmp/nix-shell.*`, an old `CARGO_TARGET_DIR`) or send the labs
to a roomy filesystem: `TMPDIR=/path/to/space cargo mutants ...`.

## A statement-prefix keyword must be transparent in *every* first-word scorer
`time [-p] <statement>` only stays one statement if each place that scores a
statement's first word sees through the prefix: the segment scanner's block-keyword
test (`keywords::keyword_delta`), the group opener's run test
(`segment::group::group_at_run`), and the group span's start (`prefix_start`). The
token list of a statement stops at the first `;`, so a prefix missed anywhere in
that set splits the block and the parse hook never sees it: `time while x; do y;
done` scanned as `Stmt "time while false"` produced `expected 'done'`. Rule: when a
new prefix form is added, enumerate the scorers of the statement's first word and
make each one strip it, then pin each with a test.

**Sharing the scorer is not enough — the *strip* must be the same rule.**
`segment::group::prefix.rs` stripped the whole prefix run with `time_prefix_len` for
the statement case and hand-stripped one `-p` plus one `time` for the cond-part case,
so `true && time time -p { sleep 0.2; }` left `time` at the end of the run: the cond
test said "not a cond part", the `{` became word data, and the group split at `;`
("group is missing a closing brace"). All three spellings work in bash. Rule: one
strip helper used by both scorers (`strip_prefix` / `strip_trailing_prefix`), pinned
per spelling of the run (`&& time {`, `&& time -p {`, `&& time time -p {`).

The scorer that *consumes* the run is the third one to keep in step, and it drifts the
same way: `parse/time_block.rs` took every `time` token plus a *single* `-p`, so
`time -p time -p :` consumed `time -p time`, left `-p` as the inner statement, and ran a
command named `-p` (exit 1; bash consumes each `time` with its own flag, exit 0). Rule:
the consuming loop must accept `time` + its own optional `-p` as a *pair*, repeated, and
pin the agreement in bytes against the measuring helper —
`parse_prefix_run_matches_time_prefix_len` asserts the inner text is exactly the line
behind `time_prefix_len(line)`, so a run of four prefixes with two flags cannot leave a
word behind in one scorer and not the other.

## A verbatim token span cannot reach a block's closing keyword
`parse::semi::verbatim(text, tokens)` spans the first token's start to the last
token's end, which is right for commands and pipelines (and carries their heredoc
bodies). For a block it is wrong: tokenization stops at the first separator, so
`while false` is all the tokens cover and the span ends long before `done`. A
timed block's text must be the *segment* span minus the prefix (`text.subslice`
from the block keyword to the text's end). Rule: use the token span for
statement-level text, the segment span for block-level text.

## `saturating_sub` hides an inverted time delta as a silent zero
`start.saturating_sub(now)` for an elapsed time compiles, passes every test that
only checks the layout, and prints `real 0.00` for a 3-second sleep. The delta is
`now - start`; with saturating subtraction the inverted order is invisible. Pin
the direction with a lower bound on a sleeping statement (`time -p sleep 1` →
`real >= 1.00`), not with a layout-only assertion.

## A CPU-time delta measured around a forked child is the fork's CPU
`time -p sleep 0.2` reports `user 0.20` in fdshell where bash reports `0.00`: the
fork/exec of the child (PATH walk, arg substitution) is the shell's own CPU and is
inside the measurement window. So a unit or integration test may pin `user`/`sys`
to `0.00` only for statements that fork nothing (a bare `time`, `:`, an `if` with
an in-process body). Asserting `user == 0.00` around a fork is a timing coin flip.

## A CPU-burn timing test measures nothing in a release build
`monotonic_advances_across_a_cpu_burn` (200M-iteration counting loop,
`core::hint::black_box`) passed every local debug run and failed under
`nix flake check`, where the crate is built in release: LLVM computes the loop's
result directly and the burn costs ~0 ms, so the clock test asserted a 5 ms
elapsed time for work that never ran. Rule: measure elapsed time against a syscall
that blocks (`libc::nanosleep`), which cannot be folded away, and bound it on both
sides (at least 10 ms slept, far less than the clock reads). The same fold hits
`utime` assertions: `unsafe/sys/tests/getrusage.rs::self_usage_succeeds` burns
`n = n.wrapping_add(1)` up to 50M with `black_box(n)` *after* the loop, so release
builds report `CpuTimes { utime: 0, stime: 2888 }` in the nix sandbox and
`nix flake check --build-all` fails the suite intermittently (task #208). A burn
must be un-foldable to be assertable: keep the accumulator inside the loop under
`black_box`, and never assert a positive CPU count for folded-away work. Fixed by
task #171: `burn_cpu()` in `unsafe/sys/tests/getrusage.rs` feeds the accumulator
through `black_box` on every iteration (self burn and forked-child burn), which
costs ~18 ms of real user time in the sandbox's release build — `utime ≈ 18_000 µs`,
four orders of magnitude above the `> 0` assertion.

The same assertion failed again in the sandbox (task #194): a `nix log` of the
fdshell `.drv` shows `CpuTimes { utime: 0, stime: 3124 }`, so
`assert!(t.utime > 0)` failed and the next run passed. Rule: how a process's CPU
time splits between `ru_utime` and `ru_stime` is the **kernel's accounting
choice**, which no test controls, so a positive-`utime` assertion is a scheduler
coin flip — self (`self_usage`) and reaped-child (`wait_pidfd_rusage`) CPU-time
assertions are on `utime + stime`. Keep the burn loop: it is what makes the
reported numbers real. Read a `> 0` sum as "the report is non-degenerate", never
as "the burn ran"; that claim belongs to the un-foldability rule above, never to
the sum.

Diagnostic (task #194, same sandbox, un-foldable burn in place):
`CpuTimes { utime: 346_962, stime: 990 }` — the burn is effective in the nix
release profile, and ~347 ms lands in `utime`. So the `utime: 0` of the failing
report was not a measured attribution flip: that run predates the task #171 burn
fix (task #97 ran 2026-10-10, #171 landed 2026-10-13) and its 3124 µs is the
folded signature recorded above (`{ utime: 0, stime: 2888 }`, ~3 ms of
process-startup system time). Attribution stays unassertable all the same: the
sum is the attribution-independent claim, and it is what both assertions make.

Repeatability measured (task #208, 2026-10-13, `master` @ `286abdb`): the
fold-proof burns are repeatably green, and the barrier's effect is measurable. Five
**fresh** sandbox executions of the check derivation — `nix build
.#checks.x86_64-linux.default --rebuild` re-runs the whole check phase, where a
plain `nix flake check` repeat is a store hit that runs nothing — each passed
4529/4529 tests with zero `FAIL` lines, both named tests among them
(`self_usage_succeeds` 17–21 ms, `wait_pidfd_rusage_reports_child_times` 18–21 ms,
against 3–4 ms for the non-burning `from_rusage` test in the same binary); plus one
fresh `nix build .#coverage --rebuild` (4532/4532 tests — its exit 1 is the
coverage report's embedded timestamps tripping nix's output-determinism check, not a
test failure), 130 targeted `cargo nextest run --release -p sys --test getrusage`
runs, 10 debug runs and 5 full release sweeps (4529/4529 each). Timed directly in
the release profile, one un-folded burn costs ~10 ms of user CPU and the same loop
with `black_box` moved back *after* the loop ~0 ms — the burn is real in release,
which is the only thing a `utime + stime > 0` assertion needs from it.

## Measure every spelling of a command you put in a classification list
`time_exec`'s `TERMINATING` list ("the statements that end the shell process, so the
report prints *before* them") carried `.`. `.` is the `source` alias: it runs the script
in the *current* shell and returns, so `time -p . slowsrc.sh` printed `real 0.00`
*before* the sourced output, while the identical `time -p source slowsrc.sh` printed
`real 0.30` after it (bash prints `sourced`, then `real 0.30`). The same list, the same
builtin, two spellings, opposite behavior — and the comment justifying the entry ("the
statement itself returns, so its own report prints normally") contradicted the entry
under it, which the README then repeated. Rule: a list that classifies commands by their
first word is a claim about each command's *behavior* — measure each one (and each alias
of it) against the shell, and never record a claim like "not anticipated" in its place.

## A shared parser must gate the flag only one builtin honors — a flag that parses and does nothing is a usage error
`netaddr::parse_net_args` is shared by `bind`, `listen` and `connect`, and it accepted
`--backlog` for all three. `bind`/`connect` never call `listen(2)`, so `builtin bind
--backlog 99 @x` succeeded with the flag silently discarded — the user reads "exit 0" as
"the backlog is 99". A flag that parses and does nothing is worse than a rejection
(STYLE.md §4.7: the error must be actionable). Rule: when a parser is shared, a flag whose
effect depends on the caller is passed in as a capability (`allows_backlog`), checked
*before* the value is consumed, and rejected with a `Suggestion` naming the builtin that
accepts it. The same file also shows the flag-shape half of the lesson: `--bind ADDR
--port N` reads as a local bind, which is exactly wrong for `connect`, where the pair names
the **peer**; the fix is a neutral alias (`--addr`) accepted by all three, not a
peer-shaped flag that would be nonsense on `bind`/`listen`.
**Corollary (mechanical):** `Suggestion` strings are pinned by the `tests/sockets.rs`
parse-error tables via `err.contains(suggestion)`, so changing one (the
missing-address text moved to `--addr ADDR --port N`) must update every pin **in the same
commit**, or the integration suite fails for a wording change.


## Rust std ignores SIGPIPE, so *both* process hand-off boundaries must clear it for the child
`fdshell` is a Rust std process, and std installs `SIG_IGN` for `SIGPIPE` at startup; `clone3` inherits the disposition **and** the signal mask. A stage that wrote to a pipe with no readers therefore got `EPIPE`, and the error path printed `cmd: write error: Broken pipe` and exited non-zero, where bash kills the stage silently and the stage reports 141. Restoring the disposition is only **half** the fix: `signalfd %v PIPE` blocks SIGPIPE in the mask as well (measured: `signalfd %p PIPE; seq 1000000 | echo got` was still noisy with only `sigaction`), so `sys::signal::reset_sigpipe` does `sigaction(SIGPIPE, SIG_DFL)` **and** `sigprocmask(SIG_UNBLOCK, {SIGPIPE})`.

Rule: the reset lives at the two boundaries where fdshell hands off to another program — the fork wrapper `fork_pidfd_cell` (the single choke point of all four shell fork sites: pipeline, launch, `$(…)`, `wait` arm) and the parent-side image replacement in `replacer/external` (which runs **in the shell process**, so `exec seq 1000000` with a closed stdout is signaled by SIGPIPE, as in bash). Per-site resets in `child_main`/`launch`/`cmd_subst`/`wait/arm` are four call sites to remember forever, one of them easy to miss; the plain test-only `fork_pidfd` deliberately stays untouched. The shell process keeps its own ignore on purpose (the REPL must survive a closed stdout; `bash -c` has `SigIgn=0` — an accepted divergence, in the README). Pin std's install in a test (`sigpipe_disposition() == SIG_IGN` in the parent): a toolchain that stops installing it must fail loudly, not silently un-fix the bug.

Test corollary: a deterministic broken-pipe test must make the writer emit **more than the pipe capacity** (64 KiB default, so ≥128 KiB here), otherwise the writer finishes before the reader closes and every assertion is a coin flip — the reported `echo hi | echo got` was noisy in 14 of 40 runs. A 50-run loop of the racy shape is a *detector*, not a flaky assertion: post-fix every interleaving is silent (the stage either writes cleanly or dies of the signal), so it fails on the broken build and cannot flake on the fixed one.

## A quoted region is a unit for *every* byte scanner, not just the tokenizer
`"$(echo "a; b")"` failed in two independent layers. The tokenizer's quoted
scanner (`parse/quotes.rs::quoted_char`) knew only `"`, `\\` and "literal byte",
so the inner `"` closed the outer region and the bytes after it went through the
*unquoted* step arm, where `)`, ` `, `|` and `<`/`>` do their normal damage. The
byte-level scanner (`scan/advance.rs::advance`) computed `bare = !in_quote &&
!in_backtick`, so inside a quote a `$(` never raised `paren_depth` and the inner
`"` toggled `in_quote` off — and `scan.rs::boundary`, `cond/boundary.rs`,
`segment.rs`, `segment/group.rs`, `comment.rs`, `brace.rs`, `repl/trailing.rs`
and `scan/heredoc` all consume `advance`. Fixing only the tokenizer made
`echo "$(echo "a; b")"` tokenize and then split the *statement* at the region's
`;`; `echo "$(printf "x && y")" && echo ok` split the cond list;
`echo "$(printf "<<EOF")"` opened a heredoc; `if echo "${u:-"a && b"}"; then
echo yes; fi` lost its `fi`.

Rule: the rule "a region is a unit" is expressed in **both** scanners, fed by one
shared scanner pair (`paren_scan::scan_at` + `region_scan::{scan_brace_at,
scan_backtick_at}`), exactly like `scan::heredoc::skip` feeds every scanner with
one helper (LESSONS #183). A fix in the tokenizer alone leaves every byte scanner
blind; a fix in `advance` alone leaves the words mis-tokenized.

Corollaries measured here: `advance`'s skip must **not** touch `in_quote` (the
outer quote stays open across the region) and must **not** touch `paren_depth`
(the region is consumed whole, not nested); and an unbalanced region returns
`None`, so the scanner falls back to byte-at-a-time — which is exactly the
pre-existing behavior for unbalanced input, never a new error class.
(`parse/quotes/subst.rs` mirrors it: an unterminated `${ … }`/backtick pushes what
it got and lets `State::finish` report the *outer* quote.)

## `pos` counts raw bytes, `cur`/`mask` count token bytes
A region reader that drops quote bytes (the `${ … }` and backtick bodies drop
them) advances `self.pos` by the **raw** consumed count while `cur`/`mask`
shorten by the dropped bytes. Getting these apart breaks two things at once:
token spans `(start, end)` feed the pre-parse rewriters (`alias_expand`,
`brace_expand::rebuild`, LESSONS #255) and `word_quoted`'s span test
`end - start > len` (#253), and the per-byte mask goes out of parity with the
word text, so a later slice indexes the wrong bit.

Rule: in the tokenizer's word state, `pos` is a raw-line offset and `cur`/`mask`
are token bytes; every new code path must keep `mask.len() == cur.len()` (one bit
per pushed byte, no bit for a dropped quote) and `pos == raw exclusive end`.
Unit-test both — do not assume them. The region scanners return the consumed byte
count (`scan_brace`/`scan_backtick` return `(body, consumed, closed)`) precisely
so the caller can advance `pos` by the raw count while pushing the shorter body.

## Quote bytes inside a region: kept in `$( … )`, dropped in `${ … }` and backticks
Whether a nested region's quote bytes belong in the word text is a **per-region
decision**, set by what consumes the body downstream:
* `$( … )` — kept verbatim: `cmd_subst` re-parses the body quote-aware, so the
  quotes are markers *there* (the existing `cmd_subst_paren_in_quotes_is_data`
  pins it); `echo a"$(echo "x y")"` → `ax y`.
* `${ … }` — dropped: `substitute/brace.rs::read_until_close` takes the braced
  content verbatim and `param_op::colon::apply` pushes it verbatim, so kept
  quotes would print literally. `"${u:-"z w"}"` must expand to `z w` (bash
  parity); kept quotes give `z w` with literal `"` around it.
* backtick — dropped, for the same reason: task #216 expands the body through
  the same `cmd_subst` path as `$( … )`, so the body text is handed to the
  child verbatim (the drop is the accepted divergence: `echo "`printf "q r"`"`
  prints `q`, bash `q r`).

Rule: write the reason next to the choice. The same byte is a marker in one
region type and content in another; a "consistent" rule applied to all three
breaks one of them.

## Mask bits, not just text: a quoted region's bytes must carry mask `true`
`"$(printf "a b")"` tokenizes to one word with the right text, and still became
two words. `substitute/words.rs` decides field splitting from `fully_quoted(mask)
== false` → split on IFS, so a region whose bytes carry mask `false` (the
unquoted path's bit) re-splits its expansion. `set -- "$(printf "a b")"` printed
`n=2`; `"${u:-"z w"}"` and `"a$(printf " b")c"` behaved the same way.

Rule: the mask bit is a *parameter of the reader*, not a constant:
`read_dollar_paren(bytes, line, quoted)` takes it from the call site — `false` on
the unquoted path (`token/step.rs`), `true` from `parse/quotes/subst.rs` (inside
double quotes). Pin the split-count divergence with words of *different* quoting
around the region (`IFS=:; builtin printf "[%s]" x$(true)":z"` → `[x:z]` stays
2 words, `"x$(printf "a b")"z` → `[xa bz]` is one): a test with only fully quoted
words cannot tell a correct mask from an inverted one.

## A word list that must keep per-word provenance expands **word by word**
`substitute/words.rs::substitute_args` returns a flat `Vec<ShortCStr>`: after a
whole-list call the word→fields mapping is gone, and with it the per-word quote
mask, the per-word `words_quoted` flag, and the per-word provenance. A `for … in`
list needs all three, so `expand.rs::expand_for_words` delegates to
`substitute_args` on `slice::from_ref(word)` — one call per word — and classifies
the provenance from the **source** bytes in that loop: `Origin::CommandOutput` is
decided by `run_origin::has_cmd_subst` on the word as written, and a whole-list
call would classify the *expanded* text, which no longer contains `$(` or `` ` ``
(`explain` then reports a literal for a `$(…)` / backtick word). Cost: a fresh
`%fd` export cache per word —
nothing cross-word depends on it. The per-word loop must not hold a `RefCell`
borrow across the call (LESSONS.md #56/#119): `substitute_args` forks inside a
`$(…)` and reborrows for `nullglob`.

## Command-substitution output takes its split protection from the word's quote mask
`for w in "`echo a b`"` iterated twice. The whole-word backtick arm of the for
list split its output with its own `split_whitespace`, hardcoded to space, tab,
CR and LF, which was wrong twice over: it split **quoted** output (bash keeps a
quoted substitution whole, spaces *and* newlines), and it ignored `IFS`
entirely — `IFS=:; for w in \`printf %s "a:b"\`` gave one word where bash gives
two, `IFS=; for w in \`echo a b\`` gave two where bash gives one, and CR split
where bash keeps it. Task #216 deleted that arm and moved the form into
`substitute/arg.rs`, so the whole argument pipeline now shares the one rule —
the arm is gone precisely because a per-surface splitter is a second copy of
the mask rule (see "A command-substitution form belongs in the argument
pipeline").

Rule: the **word's** quote mask decides, never the output's bytes — command
output carries no quote protection of its own, so an unquoted substitution goes
through the pipeline's own splitter with an empty mask
(`split_word(&out, &[], &ifs)`: nothing protected, all `IFS` bytes delimiting)
and a fully quoted one (`fully_quoted(mask)`) is one field with no split at all.
Do not key the rule on the per-word `quoted` flag: it means "the raw span
contained quotes", so it cannot express partial quoting (it would split
`"\`echo a b\`"x`) and it exists only for the quoted-empty rule.

Test design for `IFS` pins: quote the value inside the substitution body
(`IFS=:` + `` `printf %s "a:b"` ``, never `` `printf a:b` `` — a literal word is
never split, task #133); single quotes are unavailable; a real TAB
needs `printf %b` (`%s` keeps `\t` as text in both shells); and a **quoted**
backtick body must not nest quotes (the region scanner drops them — the
accepted divergence of task #216), so get a newline from several commands:
`` `printf x; echo; printf y` ``.

## A command-substitution form belongs in the argument pipeline, not in a per-surface arm
Task #216 added backtick expansion. The tempting shortcut was a per-surface arm
in the `for … in` list (`expand/backtick.rs`, which recognized a whole-word
`` ` … ` `` and ran it) — it existed only because `substitute/arg.rs` had no
backtick arm. That arm diverged: its fields skipped pathname expansion
(task #220), so `for w in` a `` `echo "*"` `` word never globbed while the
`$(echo "*")` shape did, and any second surface that substitutes (assignments,
`local`/`let`, `set --`, here-strings, `case` words, heredoc bodies) would have
needed its own copy. The fix is one arm in the substitution loop, mirroring the
`$( … )` arm: scan the body, run it through `cmd_subst::run_and_capture`, push
the output with `mask::push_expanded(out, out_mask, &output, quoted)`. Every
surface gets the form with that one arm, and the mask / split / glob /
quoted-empty / `expanded`-gate rules apply uniformly, for free.

The deleted arm's doc comment had already commanded this ("Delete this arm when
task #216 expands backticks inside `substitute_arg`") — a per-surface arm for a
form the pipeline should own is a staging device, so write that intent in the
comment and delete the arm (and its unit tests, now covered by the pipeline's)
when the pipeline arm lands. Watch the file budget: the arm pushed
`substitute/arg.rs` into the 80–90 LoC band; keep the arm lean (one call + the
`idx = consumed.get()` re-sync) and prefer a helper module
(`substitute/backtick.rs`) over growing `arg.rs` past the limit.

Rule: implement a shell expansion **once**, in the pipeline every surface
already shares; a per-surface arm is a bug generator (divergence on the rules
the pipeline applies) and a coverage liability.

## Field splitting is an expansion-only rule; a text-comparison proxy for "did it expand" is unsound
`IFS=,; printf "[%s]\n" a,b` printed `[a] [b]` and `IFS=:; set -- a:b; echo $#`
printed `2`: `substitute/words.rs` ran `split_word` on *every* non-fully-quoted
word, so a literal word's own bytes split on the current `IFS`. A mask of all
`false` means "nothing is protected", not "this word is splittable" — POSIX 2.6.5
splits the **result of an expansion**, and a word with no `$`, no `%` and no
leading `~` has no result to split: it is one field.

Rule: track the signal, never infer it from the text. `substitute_arg_marked`
takes `expanded: &mut bool` as an **output** and sets it in exactly the five
expansion arms (`~` only when `$HOME` was pushed, `%`, `$(`, `` ` ``, `$`). Every text
proxy flips a real case: with `IFS=,` and `a=,`, the word `$a` (2 bytes) expands
to 1 byte, so `out.len() != arg.len()` calls it unexpanded and yields `$,`
where bash yields one empty field; with `a=b,` the word `$a` (2 bytes) expands
to `b,` (2 bytes), so the length proxy, `mask.len() != arg.len()` and
`expanded_text != arg` all call *that* word unexpanded and keep `b,` whole
where bash yields `b`. Pin both directions: `equal_length_expansion_of_*` plus
`expansion_with_space_still_splits_on_default_ifs` (a 3-byte word whose
3-byte expansion must split).

Rule: an output flag is cleared by the callee, and its negative test starts at
the *wrong* value. The wrapper's `let mut expanded = false` is invisible to a
test that also starts at `false` — the assertion `!expanded` then passes without
the callee writing anything (the first `literal_word` pin passed for exactly
this reason). `substitute_arg_marked` clears the flag at its entry, and every
"marks no expansion" pin starts at `true`.

Rule: the trailing-empty rule is only about the very end of the expansion. The
`trailing` flag in `split_word` is load-bearing in the whitespace arm — a
non-whitespace delimiter at the end of a field merges with the whitespace run
that follows it and is trimmed with it (`IFS=" ,"; set -- $v` with `v="a, "` →
one field `a`), so drop only its use in the *final* flush
(`if !cur.is_empty()`, not `|| trailing`) and keep the flag and its three
assignments; `delimiter_then_trailing_whitespace_merges` is the pin that kills
the "delete the flag" shortcut. `split(":")` is one empty field, not none: the
leading delimiter delimits it, and the word end adds no second one (bash:
`IFS=:; set -- $(printf ":")` → `$#` = 1).

Flag `safe/fdshell/src/substitute/arg.rs` (83 LoC, STYLE.md §2.3 80–90 band) for
refactoring. The structural fix — splitting each expansion part separately and
concatenating it with the literal parts, so a literal `IFS` byte after an
embedded expansion stops cutting the word (`IFS=,; v=$(echo "b c"); printf
"[%s]" x$v,y` → `[xb c][y]` here, `[xb c,y]` in bash) — is task #222; the
`expanded` bit added here is exactly the information that refactor needs.

## A capability base that tracks the process CWD must be pinned by a test that diverges them

Strict mode's resolution base is the shell's pinned `%CWD` fd, not `AT_FDCWD`.
`cd` calls `fchdir`, so inside one process the two are the same directory and a
`base → AtFd::cwd()` mutant is *in-process unkillable*: `openat2(%CWD, rel)` and
`openat2(AT_FDCWD, rel)` open the same inode. The pin has to diverge the state:
spawn with the process CWD at a scratch root, run `cd in` (moves `%CWD` only),
then assert the effect lands in `in/` — `strict_redirect_base_follows_cd`
(`cd in; builtin echo hi > f` must create `in/f`, never the root `f`) — and for
the PATH search park a same-named decoy in the launch dir that *cannot* be
exec'd (`strict_exec_search_base_follows_cd`: root `probe` is mode 0644, so the
`O_PATH` probe succeeds and `execveat` fails) and pin the pinned program by its
exit 42. Same species as the "pin with a test that diverges the state" entries.

Rule: a ban inside a search loop must *escape* the loop. `search_path_str`
probes entries with `if let Ok(..)`; the strict absolute-`PATH`-entry ban
returns an `Err` from inside that arm, so the loop swallowed it and the search
ended in a plain `NotFound` — the ban compiled, ran, and was unobservable
(harness `PATH=".:nonexistent:/bin"` proved it). Hoist the ban to the loop head
as its own `bail!`, and pin it with a `PATH` whose relative entries miss and
whose absolute entry is reached.

Rule: a shell-behavior test must make the *shell* own the redirect. A script
body's `echo x > f` is the child shell's redirect — it tests bash, not fdshell.
fdshell is PATH-first for bare words (a builtin needs the `builtin` keyword or
`shopt -s builtin_first`), and `<>` on a bare command lands at a free fd with
no dup to 1 (bash: `exec 3<>f` is the way to write through it), so the strict
pins are `builtin echo hi > out` / `exec 3<>f; builtin echo z >&3`, run with the
harness `PATH` set to relative entries, and use helper *binaries*
(`EXEC_OK_PATH`, `ARG_PRINT_PATH`) copied into the scratch dir: no `/bin/sh`, no
coreutils dependency, deterministic exit codes.

## A keyword block is opaque to the cond-list split — and `scan_block`'s end is a *run* end, not the closer's end
`&&` / `||` inside a `while`/`until`/`if` condition, a loop body, an `if` body, a
`case` arm or a `wait` arm is the block's *own* cond list: the block executors
run a condition through `run_cond_list` (with `exempt`) and parse bodies inside
the block. So `cond/boundary::next_boundary` must not split a block text in
half — it counts the block's keywords (`cond/boundary/block.rs::word_delta`, the
shared `keywords::keyword_delta` scorer) and only splits at `group_depth == 0 &&
block_depth == 0`, exactly the rule it already had for a `{ …; }` group. The
counter's brace rule is layered the same way: inside a block (`block_depth > 0`)
`{`/`}` score `keyword_delta`'s ±1 (the rule `comment::depth_delta` uses to find
the span, so a literal `}` argument eats the closer there — the accepted
divergence in `tests/group.rs::literal_brace_argument_in_a_block_body`), and at
top level a group opener scores 0 because the group arm applies the segment
scanner's clause-position rule (`group_at_run`), keeping `echo { && echo b` a
split. A `time [-p]` prefix needs no special case: `keyword_delta` already
strips it, so the depth fires at the keyword behind it.

Two traps found while doing it:

1. **`comment::scan_block` returns the closing keyword's *separator*, not its
   word end.** Its runs break only at `;`/`\n`/end-of-input — spaces are not run
   boundaries — so a "jump over the block" built on it swallows everything
   written after the closer on that run: `while true; do a; done && echo b` jumps
   to the end of the line, the trailing `&&` disappears, and `echo b` runs
   unconditionally (and `run_one` gets `… done && echo b` as one part, a parse
   error). A depth counter over the words `next_boundary` already walks has no
   such boundary, so the scan resumes exactly where the block ends.
2. **The opacity rule must be gated on a heredoc-free text.** A block's
   condition bodies travel *inside* its span (`segment/block/keyword.rs` extends
   `end_pos` past them), so making that text opaque also hides the `&&` written
   in a *body* line, which must stay a split point:
   `tests/heredoc.rs::block_heredoc_cond_run_and_and_in_body_splits_part` pins
   the split (rc 1 `missing 'fi'`, bash rc 2) and is the alarm. The gate is
   `heredoc::operator_count(line, i, line.len()) == 0`, and the gap it leaves
   (`while cat <<X; do a && b; X` + `done`) is task #234, recorded in README.
   Task #235 owns the twin case (a `&&` in a function-definition group body,
   where the definer word keeps the `{` off the run start).
3. **A keyword scorer must ask *command position*, not word start.**
   `keywords::keyword_delta` scores `if`/`while`/`for`/`case`/`until` as `+1`
   wherever the word sits, and a word start is not a command position: the
   first `#137` round scored `echo if && echo b` as a block, so the part lost
   its `&&` and printed one argument `if && echo b`;
   `echo x > if && echo b` wrote `x && echo b` into the file named `if` and
   never ran `echo b` (data corruption). Every other layer already knows the
   rule — bash, the segment scanner, and the group arm of this very scan
   (`group_at_run`: the run is empty, or it ends with `&&` / `||`, a `time [-p]`
   prefix transparent) — so the keyword arm must call it too, with `run_start`
   passed in. Gate it on the *top level* only: inside a block (`block_depth > 0`)
   the scan cannot see `then` / `do` / `else` / `elif` / body / arm positions, so
   a nested `if` there keeps scoring, and its closers clamp the depth back to 0
   (`while true; do if true; then false; fi && echo x; break; done` is one part).
   Pins: `cond/boundary/block/tests.rs::keyword_shaped_argument_words_keep_the_split`
   and `tests/cond_blocks.rs::keyword_shaped_argument_words_do_not_swallow_the_cond_operators`.
   The gate leaves two gaps, tasks #236 (argument-position keywords *inside* a
   block body — a body needs its own clause-position rule) and #237 (comment
   lines inside a body: `# done` drops the counter and re-opens the split).

## A cond-list heredoc arm's `b == b'\n'` guard is a mutation survivor with no mid-run pin

`cond/boundary.rs` skips a heredoc's body with
`if b == b'\n' && let Some(resume) = heredoc::skip(line, run_start, i)`. The
`==`→`!=` mutant survives, and `master`'s own run of this file reports the same
mutant (`boundary.rs:37:18` there, `:60:18` after #137's comments shifted the
lines), so it is pre-existing, not #137's. Reason: every cond-list heredoc pin
the suite has puts the `<<` at a run start, so the scan reaches the newline that
closes the *same* run and the two forms agree. The `!=` form only differs for a
`<<` written **mid-run** whose body follows (`cat <<X && echo b` + body +
delimiter inside a cond list), i.e. the mid-run keyword gap task #232 owns.
Record it: when #232's mid-run work lands, add a cond-list pin for that shape
measured against bash and the survivor dies with it. (Measured with
`cargo mutants -j4 --test-tool nextest --file safe/fdshell/src/cond/{boundary,boundary/block}.rs`:
42 mutants — 40 caught, 1 unviable (the chained-`let` `&&`: `|| let` is not a
legal let chain, verified by hand), and this one missed. `master`'s own run of
the file reports the same `==`→`!=` plus two further survivors (the stray-`}`
arm's `==` and the `\n`/`;` arm's `||`) that #137's pins now kill.)

## A region window and the skip derived from it are one pair — only on the success path

`parse/cond_bodies.rs` anchors both halves of a condition's body extraction at
`line_start_of(line, cond_start)`: the **window** `operator_delims` scans for
the line's operators (their regions are laid out contiguously after the
condition line's newline, in operator order) and the **skip** `operator_count`
computes for the operators that claimed the head of that stream. Prepending `k`
operators to the window prepends `k` regions and raises the skip by exactly `k`,
so the slice `regions[skip .. skip + ops.len()]` is identical for every anchor.
The pair-shift is therefore invariant **on the success path**. Of the two
`cond_bodies.rs:77:75` mutants (the `line_start_of` anchor, at line 63 before
task #238 extracted `claimed_regions`), `n + 1` → `n * 1` is **strictly
equivalent** (`* 1` is the identity; measured twice on the quiet suite under it,
all 4606 tests green, leading-newline pin included; a `-j4` mutation run reports
it *caught*, which is load flakiness — re-measure quietly) — the file's only
accepted missed mutant; the newline byte is never
part of an operator, so on a line whose head is not `<<` the anchor shift
counts the same operators. `n + 1` → `n - 1` is that same equivalence only for
`n ≥ 1`: at `n = 0` it computes `0 - 1` in `usize` — a debug panic (release
wraps to `usize::MAX`), emptying the window — and is **killed**, not accepted,
by `condition_bodies_of_a_text_starting_with_a_newline`. A blanket "a window
starting before, at, or after the newline counts the same operators" is an
over-claim: it is exactly the species this entry's own rule below forbids.

**The missing-delimiter path is not anchor-invariant.** `body_regions_full`
(`scan/heredoc/full.rs`) returns `Err(n)` when a *windowed* operator's delimiter
line is missing and `condition_bodies` maps that to an empty list, so widening
the window to an earlier line drags that line's unterminated heredoc into the
stream and empties the condition's bodies. The `line_start_of` **literal**
mutants `→ 0` and `→ 1` are therefore *not* equivalent:
`parse/cond_bodies/tests.rs::condition_bodies_window_is_bounded_to_the_conditions_line`
(complete condition line, unterminated `<<` on an earlier line) kills both. `→ 1`
is inert only when the computed anchor is 0, i.e. only for a condition that
starts its line. The `b == b'\n'` → `b != b'\n'` mutant in the same function
(`cond_bodies.rs:77:55`) is likewise **not** equivalent: it is killed by
`condition_bodies_of_an_incomplete_line_stream_are_empty` and
`condition_bodies_skip_the_regions_claimed_before_them`.

`condition_bodies_claim_the_mid_line_region_of_a_multi_line_condition` pins the
non-trivial anchor (operator mid-line in a multi-line condition, the shape bash
rejects with a syntax error) and the exact region it claims (`cat <<A; echo x` +
`Z`, bytes 18..36); the suite is green under every pair-shift, which is the
success path's equivalence record.

**Rule:** scope an equivalence claim to the path it argues about — a shift that
is invisible once the regions are consumed is visible on the path that refuses
to consume them. And never record a mutant as unkillable from a loaded run:
measure it quietly (no parallel load, suite twice) first, and write down which
tests kill the ones that are killed.

## A catch-all short-flag parser must be the last arm of the builtin that owns it

`set` claims several flag-shaped words before its short-flag word is treated as
a cluster: `--`, `-o`/`+o`, `-x`/`+x`, `-F` (the fd list) and
`--stdout-capture-limit`. Task #142 turned the single 2-byte `-c`/`+c` mapping
into a cluster parser (`set_short.rs::short_flags`) that accepts **any**
sign-shaped word, so it has to run after every one of those arms: `F` is not one
of the cluster's `e u f v x` bytes and `--stdout-capture-limit` is a long
sign-shaped word, so moving the cluster above them silently converts two working
fdshell forms into `set: unknown option '-F'` / `set: unknown option
'--stdout-capture-limit'`. Nothing in the cluster parser changes — the defect is
pure arm order. Plain flag tests (`set -e`, `set -eu`) stay green under it, so
the only pins are the flag-shaped ones:
`intercept/tests.rs::try_intercept_set_cluster_keeps_the_flag_shaped_arms_first`
(new, pins `-F`, `--stdout-capture-limit` and the `-eF` cluster error) plus the
pre-existing `tests/set_cmd.rs::set_dash_f_lists_fd_variables` and the
`--stdout-capture-limit` tests. Rule: when a parser grows into a catch-all over
a shape, re-audit every arm that already claimed a member of that shape.

## A slice-range `get` is `Some(empty)` at the length, so write the length guard as a filter

Scanning the bytes after the sign looks like the `ulimit_cmd/parse.rs:38`
precedent: `bytes.get(1..).unwrap_or(&[])` behind a `len() >= 2` guard. That
guard makes the `None` arm **unreachable** (`get(1..)` on a slice of length ≥ 1
is `Some(&[])` when `1 == len`, never `None`), and an unreachable arm is an
unkillable equivalent mutant to be recorded, not tested around. Expressing the
same guard as a filter on the slice —
`let Some(flags) = bytes.get(1..).filter(|flags| !flags.is_empty()) else { return Ok(None) }`
— has no dead arm: the lone `-`/`+` word (length 1) takes the `else`, so the arm
is reachable and killable, pinned by
`set_short/tests.rs::signless_empty_and_lone_sign_words_are_not_clusters` and
`intercept/tests.rs::try_intercept_set_unhandled_forms_return_false` (the
`filter` → `is_empty()` mutant flips exactly those words into empty clusters).
Rule: a `get(range)` whose `None` case is excluded by a length guard above it is
dead code in disguise — fold the guard into the `Option` (filter) so the
fall-through arm is real.

## Apply cluster flags one by one: a mask sum is the disjoint-`|` trap, and `+` is a bug

The cluster's bits go into a `Vec<u32>` in word order and each is applied by its
own `options::set()` call (`set_short.rs::run_set_short`). Folding them into one
mask with `|` recreates the disjoint-flag equivalence source of the `|`→`^`
entries above, and folding with `+` is not merely unkillable but **wrong**: the
bits are powers of two, so a repeated byte carries into its neighbour —
`ERREXIT + ERREXIT` = `512 + 512` = `1024` = `NOUNSET`, i.e. `set -ee` would
enable nounset (bash: errexit twice, nothing else). Keeping the bytes as
repeated entries in a vector makes the duplicate free and idempotent by
construction; `set -ee` is pinned at both levels
(`set_short/tests.rs::duplicate_flag_bytes_stay_repeated`,
`tests/set_opts.rs::cluster_duplicate_flag_is_idempotent`), so a `+` fold fails
a test and a `|` fold has no mutant to record. Rule: for a list of disjoint
flags, accumulate a *list* and apply per item; never build a mask by arithmetic.

## The shell must not consume bytes a builtin owns: `%%` is printf's, not the shell's

`arms::percent` had a dedicated arm for a following `%`: it pushed one `%` and
**consumed the second**, a shell-level escape bash does not have. The argv that
reached `printf` was therefore `%` where bash passes `%%`, and a trailing `%` is
POSIX's *missing format character*: `printf '%%'` ended at rc 1,
`printf 'a%%b'` printed `a`, `echo '%%'` printed `%`. The arm is gone
(`substitute/percent.rs`): `%%` is two ordinary literal bytes and the
`printf` builtin (`child/printf.rs:74-77`) interprets its own escape, which is
the only semantics that can match bash — a shell collapse and bash-parity
output are mutually exclusive.

* The `%` arm consumes **exactly the trigger byte** for a non-name `%`, so
  `arg.rs`'s `realign` keeps the quote mask 1:1 for it. Pin the mask, not just
  the text: `substitute/tests.rs::percent_double_percent_keeps_the_quote_mask`.
* The literal-`%` escape is the POSIX #4.1 pair **outside** quotes
  (`\%s` → `%s`, pinned by `percent_escaped_percent_with_bound_fd_is_literal`
  and `tests/quotes.rs::percent_bytes_are_data_in_a_span`). Inside a span there
  is no escape pair, so a quoted `%name` is substituted too: with fd `s` bound
  `printf '%%s' x` becomes the format `%<fdnum>` and printf reports
  `invalid format string` at rc 1 where bash prints `%s` at rc 0 — pinned as an
  accepted divergence by `tests/printf.rs::printf_double_percent_with_bound_fd_name`
  and tracked by task #252 (the `b'%' if !quoted` gate).
* An undocumented "feature" pinned only by a unit test is a divergence to fix,
  not a contract to preserve: `percent_double_percent_is_literal` asserted the
  collapse (`a%%b` → `a%b`) and nothing in the README ever documented it.
  Measure against bash first, then re-pin the test
  (`percent_double_percent_stays_literal`), exactly as tasks #107/#109/#141 did.

## e2e tests that assert `rc == errno` must *retry* the load-dependent errnos, never skip them

Task #159: the **unmutated baseline** of `cargo mutants -j4` aborted on
`verity::double_enable_is_eexist`, and the immediate re-run was green. The mechanism is
the exit-code channel itself: `handle_builtin_error` returns `Ok(e.errno())` for a syscall
failure, so a kernel/fd transient reaches the test *as the exit code*, and the suite pins
exit codes (`17` EEXIST, `61` ENODATA, `1`, `42`). Under `cargo mutants -j4` (hundreds of
concurrent test processes doing kernel work) the work `FS_IOC_ENABLE_VERITY` does is
exactly the work that fails transiently: Merkle-tree build + fs transaction (kernel
allocations, btrfs metadata reservations → `ENOMEM`/`ENOSPC`/`EIO`/`EBUSY`), `openat2`
racing the system-wide fd table (`ENFILE`), fork/exec of the tiny child under memory
pressure (`EAGAIN`), a concurrently rebuilt target binary (`ETXTBSY`).

Rules:

- Classify the transient set **in the shared helper** and retry it with a bounded budget
  (`tests/verity.rs`: `TRANSIENT` + `run`'s 9 attempts, 8 × 10 ms sleeps), never *skip* it.
  A skip that swallows transient errnos hides the pressure, and per "A mutant 'caught' by a
  flaky test is a false kill" it corrupts the gate's accounting.
- Keep the graceful-skip set exact: only `ENOTTY`/`EOPNOTSUPP` (fs lacks fs-verity)
  skip. A transient is retried, a non-transient rc is a hard failure. No pinned rc
  (17, 61, 1, 42, …) is in the transient set, so no pin can be masked by a retry.
- Absorb spawn failures as well: `Command::output()` reports fork `EAGAIN`/`ETXTBSY` as
  an `io::Error`, never as an rc, so a `.output().unwrap()` in a helper is itself a flake.
- Retrying a *failed* enable is safe for exactly this set: the kernel sets the verity
  flag only in its final success step, so a transient failure leaves the file non-verity
  and the repeat re-runs the identical first-enable path — which is why `EEXIST` must
  stay out of the set (a half-enabled file must stay visible).
- Give `--enable` a script **of its own**, never a composite `--enable; <query>`: `;` is a
  sequence, not a stop (the script's rc is the *last* command's rc), so a composite hides a
  step-level transient. Measured with a step-level injector: a transient at the enable step
  leaves the file non-verity, the query step then prints `enabled=no` and the composite
  reports rc 0, so `run`'s retry re-runs the whole script against an inode the first attempt
  already enabled (`--enable` → `EEXIST`, and `;` swallows that too). The hard failure
  names no errno at all — `stage 1 stdout="enabled=no"`. One idempotent step per `run()`:
  enable-only, then query-only (`verify_then_exec` stage 1; the `enable_and_query_*` and
  `dump_*` tests were already shaped that way, which is why only stage 1 was exposed).
- Name the errno in the helper's output (`transient rc …, retrying`,
  `enable failed: rc=… stderr=…`) and hand the last output back to the assertion when the
  budget is exhausted: a real gate failure then identifies the errno, and the set grows
  by one constant.
- **Verify with fault injection, not with load.** A wrapper at the suite's
  `CARGO_BIN_EXE_fdshell` path that fails 1-of-N invocations with a chosen rc (and an
  interpreter symlink deleted/recreated for the spawn-failure class) reproduces the whole
  class deterministically: injected `EAGAIN`/`ENFILE`/`ENOSPC` → suite green with visible
  retries, injected `EINVAL` (22) → suite fails hard, injected `ENOTTY` on the
  **enable-bearing** invocations → graceful skip. An *invocation-level* injector cannot
  show the skip as "suite green": forced on every invocation it also breaks the tests that
  never enable (`query_non_verity`, `error_paths`, `dump_non_verity`, `dump_error_paths`),
  which legitimately fail — a verity-less host is a *step*-level fault (the ioctl), not an
  invocation-level one. A step-level injector is the only way to prove the composite-script
  rule above: run the real enable, then exit with a transient rc.
  A load loop alone is worth less: it is rare enough to hide behind other
  flaky tests (measured: 48 loaded suite runs, the verity suite never failed, two *other*
   tests did).

## A positional parameter is a parameter, not a variable name
`${1}` used to be the variable named `1` because only the bare `$` path had a digit scanner, so every braced arm (plain, pattern, colon) read `strings`/`environ` and a positional expanded to empty — and under `set -u` it bailed `1: unbound variable` for a parameter that *is* set. The fix is one `positional_value` shared by both paths, reached from the braced side through the same lookup that owns the nounset rule (the existing "unset-parameter rule is one rule shared by all four arms"), so the four families agree by construction. Two corollaries: the **write** arm must refuse exactly the same names (`:=` cannot store a positional — the write would be unobservable, because the read path now prefers the positional), and the rule reaches the operator families only if the lookup, not the arm, does the resolving. Discriminating measurements matter: bash's `${1#a}` → `[]` is the coincidence fdshell already produced (pattern `a` strips the whole value `a`, and the unset name `1` expanded to the same empty string), while `${1#c}` → `a` is the row that distinguishes the parameter from the unset name (LESSONS: "verify bash's real output before pinning a test").

A parameter name is **exclusive**: a digit run must be answered by the positional and never fall through to the variable lookup, because the inherited environment can supply the very same text as a variable name (`env 4=x`). `positional_value` reports an out-of-range index as unset, so the plain fall-through re-enters `var_value` and expands `${4}` to `x` while the bare `$4` stays empty — the two paths drift, and `:=` refuses to write a name the read arm still reads. Measuring the fix therefore needs an environment that *contains* the digit name (`env 4=x bash -c …`): a plan's ground-truth table measured in a clean environment cannot see the fall-through at all. Keep the exclusivity in the lookup (`is_digit_run` → `return positional_value(..)`), not in the arm.

The write arm's "unobservable write" rationale covers the plain names only. An indirect **target** keeps the plain lookup (the accepted D4 divergence, task #260), so `${!p:=q}` with `p=1` reads nothing, and the refusal is rc 1 `1: cannot assign in this way` where bash prints `[a]` at rc 0: there the guard's justification is "a parameter has no slot", never "the read path prefers the positional". Document that face with its own bullet and pin, and do not "fix" it by resolving the target through the parameter lookup — that is #260's declared scope.

## Special parameters render values, so the shared lookup returns owned `ShortCStr`s
`${#}`, `${?}`, `${$}` and `${@}` are counts, statuses, pids and a join — none of them is a borrow of the state, so the braced lookup cannot return `&ShortCStr` any more; it returns owned `ShortCStr`s (STYLE §6.3: `clone` is documented as very cheap) and every call site owns its value for the length of its statement. Keep the nounset flag a **parameter** of the lookup rather than reading `options` inside it: the exemption is family policy (the colon family is exempt), and a lookup that reads the flag itself silently nounset-checks the family that must not be checked. `colon::assign` is the one site where the ownership matters for borrows: the shared borrow must end at its block before the `set_var` re-borrow, with the value and the target crossing as owned values (existing "shadowing a Ref guard does not drop it" rule).

## A digit run that overflows keeps the pinned `TooLarge` bail
Sharing `positional_value` between the two paths makes the braced `${99999999999999999999}` hit the `usize` parse, and the tempting bash parity (fall through to a variable lookup, expanding to empty at rc 0) would break `tests/herestring.rs::here_string_expansion_failure_is_reported`, which pins the bare path's rc-1 `TooLarge`. Keeping the bail makes the braced form rc 1 where bash is rc 0 — an accepted divergence with a README bullet and a unit pin, chosen over a per-path parse so one index map stays honest. Rule: when two paths share a helper, audit what each path's pinned rc is *before* moving the shared logic, and prefer the documented divergence over duplicating the parse.

## The byte scan's "preceding token" test must mirror the token layer's `i != 0 && prev != |`
`scan/heredoc/ops.rs` counted a `<<` only after a *command word* (`seen_word`, set from `state.word_active`) while `parse/heredoc/operator.rs::is_operator` counts it whenever a **token** precedes it and that token is not a pipeline `|`. `is_word_break` ends a word at `>`, `<`, `|`, so a word-seen flag can *never* express the token rule: re-measured on master (`f821c94`), `> <<EOF`, `>| <<EOF`, `>> <<EOF`, `>& <<EOF`, `<> <<EOF`, `< <<EOF`, `; <<EOF`, `& <<EOF`, `&& <<EOF`, `( <<EOF`, `>> >& <<EOF`, `> << EOF`, `> <<EOF <<B` (1/2) and `> <<EOF && cat <<A` (1/2) diverged (byte 0 / token 1 where not noted), while `<<EOF` (run start), `| <<EOF`, `> | <<EOF`, `>x<<EOF`, `><<EOF`, `> "<<EOF"`, `# x\n<<EOF`, `\< <<EOF`, `>>x <<EOF`, `cat ; > <<EOF` and the multi-statement forms `cat <<A ; <<B`, `cat <<A && <<B`, `cat <<A && cat <<B`, `cat <<A | tr <<B` agreed. The fix is a flag with the token layer's meaning (`seen_token`), not a wider word rule. The measured shape of the mirror:

- `;`, `&&`, `&` and `(` **are** tokens for the tokenizer, so they set the flag: `; <<EOF` is 1 at both levels, `cat <<A ; <<B` is 2. A flag reset at `;`/`&&` was measured (task #185's planning worktree) and **rejected** — it re-diverges in the other direction (`; <<EOF` 0 vs 1, `cat <<A ; <<B` 1 vs 2). "Count agreement" pins must be measured against the token layer, never against intuition about what starts a statement.
- A pipeline `|` never sets the flag; the `|` absorbed into a `>|` clobber is operator content of its word and is decided by `precedes_pipe`/`parse::redirect::clobber_pipe`, never by this flag.
- The flag's byte is read **before** `state.advance`: the `\<`, `$(`, `((` and quoted-region arms jump 2+ bytes, so a post-jump read misses the token of a run that opens with an escape pair (`\< <<EOF` counts 1).
- Do not derive the predicate from `scan::advance::is_word_break`: that list names bytes that *end* a word, and `;`/`&`/`<`/`>` are word-enders **and** tokens.
- Whitespace never sets the flag, so the pins that kill the `is_token_byte` mutants are the **whitespace-leading runs**: `  <<EOF`, ` <<EOF`, `\t<<EOF`, `  <<EOF x`, `  <<-EOF`, `   | <<EOF` → 0 at both levels. Every other pin of this rule has a token between the run start and the `<<`, so its flag is already set by that token and a mutation that turns *every* byte into a token byte (`is_token_byte -> true`, `delete !`, `&& -> ||`) cannot be seen there. Pinned in `scan/heredoc/ops/tests.rs::redirect_operator_leading_run_counts_the_operator` and `parse/heredoc/tests.rs::operator_count_leading_operator_forms_agree`.

Rule: when two layers count the same thing, write the predicate on the layer that owns the definition (the tokenizer here) and mirror it byte-wise; a *word*-shaped approximation is equal only on the lines that happen to carry a command word. The delta is pinned, not "fixed" silently: the line-level count claims the body region, so a token-leading line now consumes its body and reports its operator word (`> <<EOF` + body → rc 1 `">" not found`, `; <<EOF` + body → rc 1 `"<<EOF" not found`) where master ran the body lines as commands; bash is a syntax error (rc 2) on all of them (task #185, `tests/heredoc.rs::heredoc_leading_*`).

## A value substituted with an empty mask folds quoted escape pairs as unquoted
The three assignment surfaces — `run_env::expand_one` (a scoped `NAME=value` prefix), `run_dispatch/assign_str.rs::set` (a bare `NAME=value` statement) and `intercept/local.rs::expand_value` (`local NAME=value`) — all called `substitute_arg(value, &[], …)`. An empty mask means every byte takes the POSIX 2.2.1 (unquoted) arm, so a `\<char>` pair written **inside double quotes** — which POSIX 2.2.2 keeps, dropping the backslash only for `\$`, `\\`, `` \` `` and `\<newline>` — folded as an unquoted escape: `x="a\ b"; printf "[%s]\n" "$x"` printed `[a b]` where bash prints `[a\ b]`, and the same folding reached a child verbatim (`x="a\ b" env` showed `x=a b` vs bash's `x=a\ b`) and a stored pattern (`${x#"["a]}` stripped the quoted `[`). All three now take the value word's **own** mask: `CommandLine::env_assigns` is `Vec<Assign>` (name, raw value, mask) and `ParsedLine::AssignStr(Assign)`/`AssignStrs(Vec<Assign>)` carry it to the bare statement; `local` slices its own word's `args_mask`. The slice offset `name.len() + 1` exists in exactly one function, `parse::envassign::value_mask`, so the three consumers cannot drift (LESSONS: slice the word, its mask and its quoted flags with the same offset); it rests on the tokenizer's 1:1 invariant (`cur.push_byte` ⇔ `mask.push`, pinned as a test in `parse/tests.rs::test_token_mask_is_parallel_to_the_word` — not as a `debug_assert!`, because nextest runs debug binaries) and pads short masks as unquoted, the same padding `substitute/arg.rs` applies. Two shapes carry the pins: `"\$b"` (the escape pair *is* the value's first byte, so a shifted slice misreads it) and the two-word pair `x="a\ b" y=b\ c cmd`, whose masks differ (`[true;4]` vs `[false;4]`) — the LESSONS rule that a same-quoting pair cannot tell an off-by-one from an off-by-two. The mask changes **folding only**: the value is stored whole (`substitute_arg` never splits and never globs — splitting lives in `substitute/words.rs::substitute_args`, which no assignment path uses), so `x="a b"` keeps its space, `x="a *"` stays a literal star, and the `&[]` → mask change cannot introduce a split; the *use* of the value still splits on IFS (`x="a\ b"` used unquoted splits into `a\` and `b`, as bash). The single-quote encoding is mask-independent (its `\$`/`\\` fold identically under both arms — `x='a\ b'` prints `[a\ b]` with and without the mask, pinned in `tests/env_assign.rs::single_quoted_value_is_mask_independent`), so task #141's claim that it needs nothing from #217 stands. The same `&[]` literal survived in `redirect/herestring.rs`, `redirect/heredoc.rs` and `case_exec.rs` (here-string word, here-doc body, `case` word) until task #269 threaded those masks too; `export` never substitutes at all (task #214).
## No substitution surface takes an empty mask: the mask travels with the text
`substitute_arg(text, mask, …)` decides the *folding* of escape pairs from the mask alone, so a mask shorter than the text pads as unquoted and an empty mask makes the whole text take the POSIX 2.2.1 arm — the shape that made task #217's assignments fold `\<char>` written inside double quotes. The three siblings #217 left behind (here-string word `redirect/herestring.rs`, here-doc body `redirect/heredoc.rs`, `case` word `case_exec.rs`) now carry their masks too, and the review check is one command: `grep -rn "substitute_arg.*&\[\]" safe/ --include=*.rs | grep -v tests` must print nothing (unit tests of the substitution itself legitimately pass `&[]`). Four shapes of "the real mask" and the rule each one taught:

- **A mask sliced from a token has exactly one owner of its offset.** The here-string offset 3 (`<<<`) lives in `parse::here_string::word_mask`, the assignment offset `name.len() + 1` in `parse::envassign::value_mask`; the attached form slices, the bare form clones the next token's mask verbatim. Pins: `cat <<<"a\ b"` (all-quoted 4 bytes) and the two-word pair `cat <<<"a\ b" b\ c`, whose masks *differ* (`[true;4]` vs `[false;4]`) — the LESSONS rule that a same-quoting pair cannot tell an off-by-one from an off-by-two. `<<<""` keeps the empty mask, which `substitute::fully_quoted` reads as quoted (an empty quoted word is one word): do not "improve" it to `vec![false; 0]`.
- **A body is a quoted context, not a token.** POSIX 2.2 "the here-document is equivalent to double-quoting" means the mask is `vec![true; body.len()]`, derived at the substitution site (no parse-layer mask exists for a body), and the body's `\<newline>` continuations must be removed in the *parse* layer (`parse/heredoc/strip.rs`), because the quoted arm of `arg.rs` keeps `\<newline>` and the mask flag would have to ripple through every caller. That pass must pair backslashes left-to-right exactly as the substitution's quoted arm pairs them, and only these three shapes say so: `a\`+newline is the continuation (`ab`), `a\\`+newline is a pair plus a body newline (two lines), `a\\\`+newline is a pair plus a continuation (`a\b`) — bash agrees on all three, and a strip that scans for a bare `\`+newline folds the second shape into the first.
- **A quoted context expands the tilde it should not.** With an all-true body mask, `substitute/tilde.rs`'s `~` arm pushed `$HOME` for a body `~` and for `<<<"~"`, where bash prints `~` (POSIX expands tildes *before* quote removal). The gate is the mask's **first** bit (`tilde::leads`), and it must stay in `tilde.rs`: `substitute/arg.rs` is at 88 tokei LoC and cannot host an inline predicate. The pre-existing mask-alignment test had to change shape (`~/a` with `[true,false,false]` *means* a literal tilde) — a mask-alignment test whose first bit is quoted is silently testing the gate, not the alignment.
- **A joined word needs its mask joined by the same loop.** `case WORD in` joins several token words with an inserted space (`semi::try_join`), so the mask join must insert a `false` bit in the *same* loop (`try_join_masked`, with `try_join` = its text) — two loops means two space rules that drift. The pin pair is `case "a\ b" in` (mask `[true;4]`) vs `case a\ b in` (mask `[false;4]`: the escape pair stays in the token text, so the word is 4 bytes `a`,`\`,` `,`b` and folds to `a b`, exactly bash's word).
- **Measuring is not optional.** The planning table's fdshell column is inferred from the `&[]` literal and was wrong in three rows: `cat <<<a\ b` already printed `a b` (the escaped space keeps the word whole — no divergence), body `\\\$(echo hi)` already printed the text `\$(echo hi)` (the pair protects the `$(`), and `case ~ in "$HOME")` never matched (fdshell substitutes no patterns, so a `$name` pattern is literal — task #280, with the quoted-escape pattern rows `case "a\ b" in "a\ b")` and `case "a\b" in "a\b")`, which the word fix turns into *new* divergences, and which #269 must not "fix" by keeping an empty mask).

## The quoted-empty rule must not fire on a fully quoted whole-word `$@`: bash's `"$@"` *is* the positional word list
`set --; set -- "$@"; echo $#` printed `1` where bash prints `0`, and `for w in "$@"` iterated once where bash iterates zero times. `substitute_args`' tail rule ("the raw span contained quotes and zero fields came out ⇒ push one empty word", task #76) fired on `"$@"` with an empty positional deque, because the word contains quotes. bash has no such word: a fully quoted whole-word `$@` *is* the positional word list, so an empty deque is no word at all. The fix is a `continue` after the push loop in the `$@`/`$*` branch of `substitute/words.rs`, keyed on `fully_quoted(mask) && !is_star`.

The discriminator is the **mask**, never the per-word `quoted` flag — that flag means "the raw span contained quotes", so `""$@` carries it too (mask all-`false`) and keying the suppression on it re-creates the bug in the other direction (see "An empty quoted region contributes no bytes, so the per-byte mask cannot say 'this word was quoted'"). Keying it on `fully_quoted` alone is also wrong: `"$*"` is fully quoted and *does* keep its single empty join word (bash: `set --; set -- "$*"; echo $#` → 1, `for w in "$*"` iterates once). The `fq && !is_star` pair is the whole rule, and the empty deque's word counts are the **minimum pin triple**: `"$@"` → 0, `"$*"` → 1, `""$@` → 1. A test with only one of the three cannot tell the fix from the regression it must not cause, so pin all three (unit: `substitute/tests.rs::fully_quoted_dollar_*`, integration: `tests/wordsplit.rs::fully_quoted_dollar_at_*` plus the `""$@` pins beside them).

**Probe rule:** measure the word count with `echo $#`, a `for` loop, or `arg_print`'s argv count (`tests/bins/arg_print.rs` prints `[arg] ` per argv entry, so zero args print a bare newline). Never `printf "[%s]\n" "$@"` — printf reuses its format with empty substitutions, so it prints `[]` **in bash too** and reads as agreement (it did exactly that in the planning measurement). The word-0 surface inherits the rule through `command_word::args::consume` (an empty field list takes the next word), so `set --; "$@" echo hi` runs `echo hi` (bash-equal); with nothing after it, `set --; "$@"` is the pre-existing empty-command-name divergence, not new behaviour.

**Measure the surface's own pipeline before probing through it.** The plan's builtin probe was `set --; cd "$@"; echo rc=$?; pwd` → HOME, assuming `cd` receives expanded words. It does not: `intercept/cd.rs` hands `cmdline.args` — the raw parsed words — to `cd::cd`, so `cd "$@"`, `cd $1` and `cd "$HOME"` all look up the literal text on master (measured: ENOENT reports for every form, with and without positionals). `cd` therefore cannot probe this fix at all; the pipeline/`become`/`timeout` shapes with `arg_print` are the bash-comparable builtin/external probes. `cd`'s missing word expansion is its own divergence (tracked as a task, not fixed here — it is a `cd`-local substitution gap, orthogonal to the word count).

## A peeked byte must not count as consumed
`mask::Counting` counted bytes for the substitution loop, and it was wrapped in
`Peekable`: `Peekable::peek()` pulls through `Counting::next()`, so every byte a
helper only *looked at* was already in `consumed` — and `realign` assigns
`consumed` to the loop's `idx`. `idx` therefore ran **one byte ahead of the mask**
after every arm that peeks, and `mask.get(idx)` handed the byte that directly
follows an expansion its *neighbour's* mask bit. The visible shape (task #223):
`a%%b` read the second `%` at mask index 3 instead of 2, and any quantity
computed from `idx` at the loop top (task #223's name cap) is off by one.

Fix shape: the counting iterator owns its peek buffer (`Counting { inner,
consumed, peeked }`, `peek()` fills the buffer without counting, `next()` takes
from it first), so `consumed` is exactly the index of the next unread byte and
the loop's documented `idx == consumed` invariant is true rather than
approximate. Consequences worth knowing before touching this again:

- The stream type is now fixed to the word's bytes
  (`Copied<slice::Iter<'_, u8>>`), so substitution helpers take `&mut
  mask::Counting<'_>` instead of `&mut Peekable<impl Iterator<Item = u8>>`.
  Helpers that only ever call `next()` (`paren_scan.rs`) take
  `&mut impl Iterator<Item = u8>` and stay callable from both worlds; the
  generic `I: Iterator<Item = u8>` parameters that only existed to build a
  `Peekable<Counting<I>>` (`subst_paren.rs`) disappear with the wrapper.
- `Counting` carries its own `Cell` and exposes `consumed()`, so the caller no
  longer borrows a cell through the iterator; `realign`'s second parameter is
  the stream (`*idx = peek.consumed()`), which makes the coupling explicit.
- The accounting change is **global**: every arm's mask bits for the bytes
  following an expansion change from neighbour-bit to exact-bit at once. Expect
  state-asserting unit tests to be the ones that move; run the whole suite with
  `--no-fail-fast` (LESSONS: "grep is not enough") and re-derive any moved pin
  from bash/POSIX — never encode the old index.
- The two faces of task #223 compose: with the accounting fixed alone the bytes
  after an expansion keep their own bits, and with the name cap alone every
  quantity derived from `idx` is one byte off. Pin both faces (unit:
  `substitute/tests.rs::counting_ignores_peeked_bytes` plus the region-cap
  tests; integration: `tests/wordsplit.rs::quoted_expansion_glued_to_*`).

## A bare name is capped at the quote region of its trigger byte
bash ends a `$name`, a `%name` and a positional `$N` at the closing quote of the
region that contains the `$`: `"$v"bc` is the parameter `v` plus the literal
`bc`, and `"$1"2` is `$1` plus `2`. fdshell's name scans were greedy over the
word text, so they read `vbc`/`12` and swallowed the literal bytes with the
name. The cap is a **region** cap, not a length cap: `mask::region_len(mask,
from)` is the length of the run of mask bits equal to the bit at `from`, and the
arms get `limit = region_len(mask, idx).saturating_sub(1)` computed at the loop
top, where `idx` is the trigger byte's own index (before the arm peeks — see the
sibling rule above, which is what makes that index trustworthy).

- **Greedy stays greedy inside the region.** `"x$vbc"` is `VBC`; the unquoted
  `$vbc` is `VBC` too. The cap only forbids crossing the region boundary.
- **A missing mask bit is no information.** `region_len` returns `usize::MAX`
  when `mask.get(from)` is `None`, and in production that only happens where the
  mask is **shorter than the word text**: the padding rule (`arg.rs` reads a bit
  past the mask as unquoted) makes the rest of the word unquoted, and an
  unbounded scan is exactly bash's unquoted behaviour. Unit calls that pass
  `&[]` are the other caller. Callers with a real mask never take the missing-bit
  branch: a heredoc body passes `vec![true; body.len()]` and `case_exec` passes
  its joined word mask, so each is one full run and stays greedy by the *run*
  rule. Do not go looking for a substitution caller that passes an empty mask —
  there is none (LESSONS: "No substitution surface takes an empty mask").
  Pinned: `region_len_measures_the_trigger_byte_region`, and the heredoc row
  `$v"bc"` → `q"bc"`.
- **`limit == 0` means the next byte starts a new region**, so the trigger is
  literal and consumes nothing: `$"v"` → `$v`, `"$"v` → `$v` (bash prints `v`
  for the ANSI-C/locale `$"…"` family — the divergence the `$'x'` → `$x` bullet
  already documents — and `$v` for `"$"v`, so that spelling is parity).
  `expanded` stays **set**: the arm ran, which is what
  `bare_dollar_marks_expansion` pins; do not "clean up" the flag here.
  The `%` arm needs no special case: an empty name takes the existing
  literal-`%` fallback and consumes nothing.
- **A boolean mask cannot express adjacent quoted regions.** `"$v""b"` is the
  text `vb` with an all-true mask, so the run is one region and the name keeps
  collecting: fdshell prints `[VB]`, bash `[qb]`; `"$1""2"` gives `[]` where
  bash gives `[A2]`. Accepted divergence (task #285 owns the region-id model),
  pinned in `tests/quotes.rs::adjacent_quoted_regions_keep_collecting_the_name`
  with a README bullet, so #285 has the expectations to delete.
- **Bind every name a mask-alignment test probes** (LESSONS): with `vb`
  unbound, `"$v"b` prints `[]` for the greedy scan, for the cap *and* for the
  byte loss, so a test that binds nothing measures nothing. The pins glue a
  literal to a **name prefix** (`vb=VB` next to `"$v"b`) — that pair is what
  distinguishes a capped scan from a greedy one.
- **Re-measure the plan's table against bash; two of its rows were wrong.**
  The plan predicted bash `v` for `"$"v` (bash prints `$v` — parity, so the
  pin is a parity pin, not a divergence) and `"${v}"x` → `VX` for the
  adjacent-region divergence (the braced form is unaffected: the name ends at
  `}`, both shells print `[qx]`; the bare-name spelling `"$v""b"` is the shape
  that diverges). A pin typed from a plan table, not measured, pins a
  prediction.
## `dollar_subst`'s digit arm is load-bearing: a name arm that also accepts digits is not its equivalent
`cargo mutants` deletes `substitute/dollar.rs`'s `Some(c @ b'0'..=b'9')` arm and the whole
suite passes, and the arm looks redundant: the generic name arm below accepts digits
(`is_ascii_alphanumeric`) and the shared lookup routes a **pure digit run** to the positional
(`param_value` → `direct_value` → `is_param_name` → `positional_value`, `resolve/param_name.rs`),
with `collect_name(peek, limit)` capping the run at the same `limit` as
`resolve_positional_index`'s `while num.len() < limit`. The argument only covers runs that are
all digits. Measured on task #223's branch (arm deleted, `target/debug/fdshell` against bash
5.3.9, `/home/damir/tmp/opencode/mut223_table.py`): 8 of 10 probe shapes change, and the shell
goes from bash-agreeing on 8 of them to bash-agreeing on 2.

| shape (`set -- A B;` prefix) | arm present | arm deleted | bash |
| --- | --- | --- | --- |
| `printf "[%s]" "$1a"` | `[Aa]` | `[]` | `[Aa]` |
| `printf "[%s]" "$1abc"` | `[Aabc]` | `[]` | `[Aabc]` |
| `printf "[%s]" $1a` | `[Aa]` | `[]` | `[Aa]` |
| `printf "[%s]" "$1_2"` | `[A_2]` | `[]` | `[A_2]` |
| `set -u; printf "[%s]" "$1a"` | `[Aa]`, rc 0 | `1a: unbound variable`, rc 1 | `[Aa]` |
| `set -- "$1a"; printf "[%s]\n" "$@"` | `[Aa]` | `[]` | `[Aa]` |
| `printf "[%s]" $"1"` | `[A]` | `[$1]` | `[1]` |

bash ends a positional reference at the first non-digit, so the digits are the index and the
name bytes after them are literal text; a scan that reads the whole name looks up the parameter
`1a`, which is unset (empty, and a `set -u` bail). The mutant is therefore **observable** — a
missed mutant is a *test gap*, not an equivalence, and the fix is a pin, not a record. The old
pins could not see it because they glue their literal **outside** the trigger byte's region
(`"$1"2`, `"$1"abc`, `$1"2"`): the region cap truncates the run to one digit there, which is
exactly the reading the name arm produces too. Pinned inside the region by
`tests/positional.rs::positional_digit_run_ends_at_a_name_byte`; the `delete match arm` mutant is
killed by it.

Two rules. (1) An arm is not redundant just because a catch-all arm accepts its bytes: check what
the catch-all does with **more** than the arm's byte class, not with the same bytes. (2) An
equivalence claim for a mutant is a measurement over shapes, never an argument over the shared
lookup — record it as accepted-equivalent only after a probe corpus shows no difference (the
sibling entry below is one that genuinely has none). The `$"1"` row stays unpinned on purpose: it
is the accepted `$"…"` ANSI-C/locale family (`$"v"` → `$v`, bash `v`), and in that family the digit
arm ignores the region cap the name arm honours (`[A]` where the name arm's `limit == 0` rule gives
`[$1]`), so touching the arm needs that family resolved first.
## `%name`'s guard is an accepted equivalent mutant: `collect_name` self-guards the name bytes
`substitute/percent.rs`'s arm guard `Some(c) if c.is_ascii_alphanumeric() || c == b'_'` mutates to
`true` and survives, because the guard is enforced a second time inside `collect_name`'s own loop:
a non-name byte after `%` yields the empty name, `write_fd(&"")` is false because **no path binds
an fd to the empty name** (the binding callers are `cd`, `state/setters`, `wait/arm/reap`,
`run_dispatch{,/array_ops}`, `capture/commit`, `intercept/recvmsg` — whose slot names start with
`%` — `intercept/signalfd_cmd`, `for_run`), and the arm then pushes `%` plus the empty name having
consumed nothing — byte-for-byte the `_ => out.push(c"%")` fallback. Probed on task #223 with the
guard mutated over 12 shapes (`%`, `%%`, `a%%b`, `%-b`, `%)`, `"%a"b`, `%$v`, `v=q; %$v`, `%?`,
`"%%"b`, `%a%%b`, `"%v"%w`): **zero** differences, so no test can distinguish the arms. Accepted
equivalent mutant, the same species as the disjoint-flag `|` entries: record it, do not test
around it. The comment in `percent_subst` states the empty-name invariant the fallback leans on —
if a future surface ever binds an empty name, that fallback stops being equivalent.
