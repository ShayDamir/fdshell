# TODO

## fdshell

- [x] Build `child.rs` — child process logic: dup2 onto SHELLFD, resolve `%var` references,
  call builtin exec via parse + and_then, exit
- [x] Build `launch.rs` — parent logic: socketpair, `fork_pidfd()`, `wait_pidfd()`,
  returns `(WaitStatus, Fd)` capture socket
- [x] Build `capture.rs` — `Capture { var, tag, force }`, `do_captures` loop
  with tagged-matching + positional fallback
- [x] `LocalFd::verify()`, `ImportedFd::verify()`, `ExportedFd::verify()` → `Result<(), i32>` using `cvt`
- [x] REPL loop: read line from stdin, `parse()`, `launch()`, handle captures/exit
- [x] Background processes: `&>&name` stores background task in pidvar `name`, `wait &name` reaps it
- [x] External command execution via `execveat` in child
- [x] `become` builtin: process‑replacing exec with redirect support. Impl in `replacer.rs`, dispatch in `run.rs`
- [x] Split `execveat2` into `exec_fd` (fd + AT_EMPTY_PATH) and `exec_at` (dirfd + pathname)
- [x] `become` now dispatches same as child: `builtin` prefix → `dispatch_builtin()`, else PATH‑resolve → `exec_fd()`. Always calls `exit()`.
- [ ] Add `exec_fd`/`exec_at` to `safe/builtins/` crate (parse modules + integration tests)
- [x] Refactor `replacer.rs` — now 52 lines (under 80 limit)
- [x] Refactor `run.rs` — major prior refactoring done; now 81 code lines, 1 over limit
- [ ] File length: `run.rs` (81 code lines, 1 over), `script.rs` (81 code lines, 1 over) — minor extractions to get under 80
- [x] File-path redirects: extend `parse_redirect` to handle `[N] > path` / `[N] < path`, open file in parent, dup into child
- [x] Non-blocking socketpair + drain loop in `do_captures`: replace blocking `recv_fd` with non-blocking drain (EOF + `EAGAIN` → break)
- [x] Pipeline syntax `\|`: tokenizer recognizes `|` as separator (unless part of force capture `%>|%var`), parser builds `Pipeline { commands }`, `pipeline::launch_pipeline` creates pipes + per-command capture sockets
- [x] Builtin `umask` — executes at fdshell level like `unset`, affects all subsequent children
- [ ] Drop `no_std` on `unsafe/sys` — replace `IoVec`/`IoVecMut` with `std::io::IoSlice`/`IoSliceMut`
- [x] Refactor `repl.rs` (87 lines → 25 lines after extraction) — moved `run_script`/`run_cond_list`/`find_fi_end` to `script.rs`/`cond.rs`
- [ ] Refactor `if_block.rs` (95 code lines, 15 over) — small helper extraction or inline simplification
- [ ] Refactor `script.rs` (81 code lines, 1 over) — extract `is_if_or_fi` into helper module
- [x] Nested if support (depth-tracking in `run_script`, whitespace sub-word split for `if`/`fi` detection)
- [ ] Add tests for `run.rs` else/elif execution paths (currently untested: `if false; then ...; else ...; fi` chains)

