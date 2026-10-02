//! The commands the shell handles in-process (the `intercept` table in
//! `intercept.rs` plus the parse-level `umask`/`unset`).

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
    (b"let", b"Evaluate arithmetic expressions"),
    (b"read", b"Read a line into a variable"),
    (
        b"recvmsg",
        b"Receive a payload and fd vars from an AF_UNIX socket",
    ),
    (b"send_fd", b"Send an fd to the capture socket"),
    (
        b"sendmsg",
        b"Send a byte payload plus fd vars over an AF_UNIX socket",
    ),
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
