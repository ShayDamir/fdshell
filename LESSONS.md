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

## Never hold a ForkCell borrow across a recursive call that re-borrows
`nest::deeper` increments the depth, drops the borrow, runs the body (which borrows the same cell), then re-borrows to decrement. Holding the `RefMut` across the body would clash with the body's own borrows.

## A RefCell borrow conflict can masquerade as an unrelated domain error
`nest::deeper` bailed with `NestingTooDeep` both when the depth limit was hit AND when `borrow_mut()` failed (a RefCell still borrowed). `substitute_args` held a shared `cell.borrow()` across the whole arg loop, so any `$(` command substitution inside a command arg hit `borrow_mut()`, got a borrow conflict, and was misreported as "nesting too deep." Fix: scope the borrow to the narrowest use (only the `$@`/`$*` branches needed `state.positional`). General rule: when a `borrow_mut()` fallback bails with a domain error, a held borrow elsewhere surfaces as that unrelated error and is hard to diagnose.

## `|` on disjoint bit flags produces equivalent mutants
`O_WRONLY | O_CREAT | O_TRUNC` mutates `|`→`^`, which is unkillable because the O_* flags are disjoint single bits (`|` ≡ `^` ≡ `+` on them). Sum the flags with `+` instead: same value, but `+`→`-`/`*` mutants change the value and are killable by an exact-value test. See `RedirectDirection::open_flags`.

## `while cond` with an invariant condition produces equivalent mutants
In `read_paren_expr`, `while depth > 0` was always true at the loop top (depth starts at 1 and the only path to 0 breaks immediately), so `>`→`>=` was unkillable. A loop whose exit is fully decided by `break`/`bail!` inside the body should be a bare `loop`; the redundant condition is both dead logic and a mutant source. See `parse/backtick.rs`.

## Redundant boundary ±1 produces equivalent mutants
In `tokens_to_for`, `get(in_pos + 1..do_idx - 1)` adjusted bounds the surrounding code already guaranteed: `find_preceded_by_semi` only returns a `do` preceded by `;`, and `trim_semi` strips that `;` anyway — so the trailing `- 1` was unkillable (`-`→`/` is identity). When a later step normalizes what an offset excludes, drop the offset. Likewise, scanning from `in_pos` instead of `in_pos + 1` is equivalent when the token at `in_pos` can never match the needle. The same pattern in `tokens_to_loop` (`done_idx - 1` on the body slice) was worse: `done` is *not* guaranteed to be `;`-preceded, so the offset silently dropped the last body token in `while …; do cmd arg done`. Dropping it fixed a real off-by-one, not just the mutant.

## Cap test memory with a nextest wrapper script, not shell `ulimit`
`ulimit -v` in the shell caps cargo/rustc too, which need more VA than the tests. Nextest wrapper scripts (`.config/nextest.toml` with `experimental = ["wrapper-scripts"]`) wrap only each test binary: `prlimit --as=134217728 --`. An allocation blow-up (infinite-allocation mutants under cargo-mutants) then aborts that one test instead of tripping the host OOM killer, which was killing unrelated processes and corrupting whole mutant runs. The cap must stay above legitimate peak VA (this suite peaks well under 128MB). Nix builds need `util-linux` in `nativeBuildInputs` — stdenv has no `prlimit`, and its absence fails every test with exec ENOENT.

## Test fork-children must `sys::exit`, not `std::process::exit`
`std::process::exit` runs atexit handlers, which flush Rust stdio by taking its mutex. If the fork happened while another thread (the libtest main thread writing progress) held that mutex, the child inherits it locked and deadlocks in `exit` on the futex — the parent's `wait_pidfd` then blocks forever. Symptom: one random fork-based test times out, passes in isolation, different victims each run. Fix: exit test fork-children with `sys::exit` (raw `_exit`, skips atexit) — already required for production fork-children in `pipeline/mod.rs`. Remaining caveat: running one test binary with `--test-threads>1` still races on other process-wide state (env, cwd); nextest avoids this by running each test in its own process.

## Ignoring `close()` errors masks over-counted `SCM_RIGHTS` fds in `recv_fd`
`recv_fd` computes `nfds = (cmsg_len - sizeof(cmsghdr)) / sizeof(i32)` and closes every fd after the first. The `/`→`*` mutant over-counts (1 fd → 16) and `close()`s adjacent stack bytes. With the `close()` return ignored, the invalid fds only yield discarded EBADFs and the first (real) fd is kept either way — no observable difference, unkillable. Propagating the error (`.change_context(RecvFdError::Never)`, per §4.4 discarding errors is forbidden) turns the first EBADF into a returned `Never` and the existing success-path test kills the mutant. Kernel-delivered fds always close cleanly, so the check costs nothing on the happy path.

## `recv_fd` cmsg-type guard `&&`→`||` is equivalent — compare tuples instead
The `else if level == SOL_SOCKET && ctype == SCM_CREDENTIALS` branch is only reached by cmsgs the kernel sends on a Unix socket, and the kernel delivers only `SOL_SOCKET` `SCM_RIGHTS`/`SCM_CREDENTIALS` there: verified empirically that `SO_TIMESTAMP`/`SO_TIMESTAMPNS` append nothing on AF_UNIX streams, sender-side cmsgs of other `SOL_SOCKET` types are rejected with `EINVAL` at `sendmsg`, and cmsgs of other levels are silently dropped, never delivered. The mutant was genuinely unkillable — so, as with disjoint flag `|`→`+`, remove the equivalence source: compare tuples, `(level, ctype) == (SOL_SOCKET, SCM_RIGHTS)`. The `==`→`!=` mutants that replaces it with are killable by the existing `NoFd`/`PidMismatch` tests.

## Test `argv[0]`-keyed behavior (busybox mode) by re-execing `/proc/self/exe`
To exercise a code path keyed on the invocation name — e.g. busybox-style builtin dispatch — re-exec the running binary under a different name without creating a symlink: `builtin openat2 --flags O_RDONLY /proc/self/exe %>%exe; builtin exec_fd %exe <name> [args]`. `exec_fd` sets `argv[0]` to `<name>`, so the re-exec'd process behaves as that builtin. A plain symlink named `<name>` also works and, because `argv[0]` carries the full path, additionally exercises the last-`/` basename split.

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
