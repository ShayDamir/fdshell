//! The `help` command's two lists, kept in sync with the dispatch tables:
//!
//! - `SHELL_CMDS`: commands the shell handles in-process (the `intercept`
//!   table in `intercept/mod.rs` plus the parse-level `umask`/`unset`).
//! - `BUILTINS`: the `DISPATCH` table in `child/dispatch.rs` plus the
//!   `import_fd`/`export_fd` fd-pass builtins in `child/fdpass.rs`.
//!
//! The unit tests in `help/tests.rs` and `tests/help.rs` assert this holds.

pub(crate) const SHELL_CMDS: &[(&[u8], &[u8])] = &[
    (b"alias", b"Define or list aliases"),
    (b"unalias", b"Remove aliases"),
    (b"become", b"Replace shell with command"),
    (b"cd", b"Change directory"),
    (b"envfilter", b"Filter env vars for child processes"),
    (b"eval", b"Run arguments as a script"),
    (b"exec", b"Replace shell with command (alias for become)"),
    (b"exit", b"Exit shell (alias: quit)"),
    (b"export", b"Set or list exports"),
    (b"export_fd", b"Export fd to variable"),
    (b"hash", b"Show or cache path lookups"),
    (b"read", b"Read a line into a variable"),
    (b"send_fd", b"Send an fd to the capture socket"),
    (b"set", b"Set or show options and variables"),
    (b"shift", b"Shift positional parameters"),
    (b"shopt", b"Toggle shell options"),
    (b"signalfd", b"Trap signals as an fd source"),
    (b"source", b"Run a script file (alias: .)"),
    (b"timeout", b"Run a command with a deadline"),
    (b"ulimit", b"Get or set resource limits"),
    (b"umask", b"Set or show file mode mask"),
    (b"unset", b"Remove variable"),
    (b"waitpid", b"Wait for a background task"),
];

pub(crate) const BUILTINS: &[(&[u8], &[u8])] = &[
    (b"[", b"Test expression (alias for test)"),
    (b"echo", b"Print arguments"),
    (b"eventfd", b"Counter fd that arms on non-zero"),
    (b"exec_at", b"Execute with path lookup"),
    (b"exec_fd", b"Execute with fd lookup"),
    (b"explain", b"Show provenance of a variable"),
    (b"false", b"Exit with failure status"),
    (b"fchmod", b"Change file mode"),
    (b"fdexplain", b"Show provenance of an fd variable"),
    (b"fsync", b"Flush an fd to storage"),
    (b"ftruncate", b"Truncate a file to a length"),
    (b"help", b"List available commands"),
    (b"import_fd", b"Import an fd from the parent"),
    (b"lseek", b"Move the offset of an fd"),
    (b"mkdirat", b"Create directory"),
    (b"openat2", b"Open file"),
    (b"pipe", b"Create pipe"),
    (b"printf", b"Format and print arguments"),
    (b"pwd", b"Print working directory"),
    (b"renameat2", b"Rename/move file"),
    (b"resolve", b"Resolve fd variables"),
    (b"test", b"Test expression (also '[')"),
    (b"timerfd", b"Timer as an fd source"),
    (b"true", b"Exit with success status"),
    (b"type", b"Show how a command name resolves"),
];
