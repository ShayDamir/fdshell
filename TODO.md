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
- [ ] Add REPL loop reading commands from stdin
- [ ] External command execution via `execveat`

