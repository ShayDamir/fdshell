# TODO

## fdshell

- [x] Build `child.rs` — child process logic: dup2 onto SHELLFD, resolve `%var` references,
  call builtin exec via parse + and_then, exit
- [x] Build `launch.rs` — parent logic: socketpair, `fork_pidfd()`, `wait_pidfd()`,
  returns `(WaitStatus, Fd)` capture socket
- [x] Build `capture.rs` — `Capture { var, tag, force }`, `do_captures` loop
  with tagged-matching + positional fallback
- [x] `Fd::verify()` / `DupFd::verify()` → `Result<(), i32>` using `cvt`
- [x] Split `DupFd` into `dupfd.rs`
- [x] REPL loop: read line from stdin, `parse()`, `launch()`, handle captures/exit
- [ ] Background processes: `background: true` in `CommandLine` should skip `wait_pidfd` in parent, store pidfd in `%!`
- [x] External command execution via `execveat` in child
- [ ] File-path redirects: extend `parse_redirect` to handle `[N] > path` / `[N] < path`, open file in parent, dup into child
- [x] Non-blocking socketpair + drain loop in `do_captures`: replace blocking `recv_fd` with non-blocking drain (EOF + `EAGAIN` → break)

