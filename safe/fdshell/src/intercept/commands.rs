//! Intercepted command names — the shell-command half of the `help` output.
//!
//! Every command matched in `intercept/mod.rs::try_intercept` (except the
//! `quit`/`.` aliases of `exit`/`source`) must appear here so `help` and its
//! tests can enumerate them.

pub(crate) const INTERCEPTED_COMMANDS: &[&[u8]] = &[
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
    b"read",
    b"send_fd",
    b"set",
    b"shift",
    b"shopt",
    b"signalfd",
    b"source",
    b"timeout",
    b"ulimit",
    b"unalias",
    b"waitpid",
];
