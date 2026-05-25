# TODO

## Code quality

## fdshell

- [x] Build `child.rs` — child process logic: dup2 onto SHELLFD, resolve `%var` references,
  call builtin exec via parse + and_then, exit
- [x] Build `launch.rs` — parent logic: socketpair, `fork_pidfd()`, `wait_pidfd()`
- [ ] Build `capture.rs` — `Capture { tag: Option<CString>, var: CString }`,
  parse `%>foo` and `%rd>%server` from token stream
- [ ] Wire up `%CWD` as real fd (already done in main.rs)
- [ ] Add REPL loop reading commands from stdin
- [ ] External command execution via `execveat` (later)

## Syscall wrappers

- [ ] `execveat` wrapper for external commands (when needed)
