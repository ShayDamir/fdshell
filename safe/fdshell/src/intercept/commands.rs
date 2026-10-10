//! Intercepted command names — the shell-command half of the `help` output.
//!
//! Every command matched in `intercept.rs::try_intercept` (except the
//! `quit`/`.` aliases of `exit`/`source`) must appear here so `help` and its
//! tests can enumerate them.

pub(crate) const INTERCEPTED_COMMANDS: &[&[u8]] = &[
    b":",
    b"alias",
    b"become",
    b"cd",
    b"envfilter",
    b"eval",
    b"exec",
    b"exit",
    b"export",
    b"export_fd",
    b"hash",
    b"let",
    b"local",
    b"read",
    b"recvmsg",
    b"send_fd",
    b"sendmsg",
    b"set",
    b"shift",
    b"shopt",
    b"signalfd",
    b"source",
    b"times",
    b"timeout",
    b"ulimit",
    b"unalias",
    b"wait",
    b"waitpid",
];

/// Every word the dispatch table (`intercept/dispatch.rs`) handles, including
/// the `quit`/`.` aliases of `exit`/`source` (which `help` does not list).
/// Keep this in sync with the dispatch table: a new command there must appear
/// in `INTERCEPTED_COMMANDS` (or be an alias), or `run/parent.rs` sends it to
/// the forked launch path and the builtin never runs in the shell.
pub(crate) fn is_intercepted(cmd: &[u8]) -> bool {
    INTERCEPTED_COMMANDS.contains(&cmd) || cmd == b"quit" || cmd == b"."
}

/// The intercepted commands that run in the shell process, so their
/// redirections are scoped and restored when the command finishes. The
/// process-replacing `exec`/`become` family is excluded: the process image is
/// replaced, so there is nothing to restore.
pub(crate) fn is_in_process(cmd: &[u8]) -> bool {
    is_intercepted(cmd) && cmd != b"exec" && cmd != b"become"
}
