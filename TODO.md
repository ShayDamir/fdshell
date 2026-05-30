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
- [ ] Background processes: `background: true` in `CommandLine` should skip `wait_pidfd` in parent, store pidfd in `%!`
- [x] External command execution via `execveat` in child
- [x] `become` builtin: process‑replacing exec with redirect support. Impl in `replacer.rs`, dispatch in `run.rs`
- [x] Split `execveat2` into `exec_fd` (fd + AT_EMPTY_PATH) and `exec_at` (dirfd + pathname)
- [x] `become` now dispatches same as child: `builtin` prefix → `dispatch_builtin()`, else PATH‑resolve → `exec_fd()`. Always calls `exit()`.
- [ ] Add `exec_fd`/`exec_at` to `safe/builtins/` crate (parse modules + integration tests)
- [ ] Refactor `replacer.rs` (95 lines, 15 over limit) — split `execute()` into redirect‑open and dispatch helpers
- [ ] Refactor `run.rs` (88 lines, 8 over limit) — extract builtin dispatchers (`cd`, `exit`/`quit`, `become`) into inline helpers or separate file
- [x] File-path redirects: extend `parse_redirect` to handle `[N] > path` / `[N] < path`, open file in parent, dup into child
- [x] Non-blocking socketpair + drain loop in `do_captures`: replace blocking `recv_fd` with non-blocking drain (EOF + `EAGAIN` → break)
- [x] Pipeline syntax `\|`: tokenizer recognizes `|` as separator (unless part of force capture `%>|%var`), parser builds `Pipeline { commands }`, `pipeline::launch_pipeline` creates pipes + per-command capture sockets
- [ ] Builtin `umask` — executes at fdshell level like `unset`, affects all subsequent children
- [ ] Drop `no_std` on `unsafe/sys` — replace `IoVec`/`IoVecMut` with `std::io::IoSlice`/`IoSliceMut`

