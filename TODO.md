# TODO

## Code quality

## fdshell

- [ ] Build `child.rs` — child process logic: set up SHELLFD (dup2 child_fd to fd 3),
  resolve `%var` references via `dup`, substitute in argv, call builtin exec, exit
- [ ] Build `capture.rs` — `Capture { tag: Option<CString>, var: CString }`,
  parse `%>foo` and `%rd>%server` from token stream
- [ ] Build `launch.rs` — parent logic: create socketpair, `fork_pidfd()`,
  `wait_pidfd()`, receive fds from SHELLFD, store in Vars
- [ ] Wire up `%CWD` as real fd (already done in main.rs)
- [ ] Add REPL loop reading commands from stdin
- [ ] External command execution via `execveat` (later)

## Syscall wrappers

- [ ] `execveat` wrapper for external commands (when needed)
