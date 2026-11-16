// mutants: -C --lib -C --test -C alias -C --test -C cd_expand -C --test -C colon -C --test -C group -C --test -C hash -C --test -C help -C --test -C heredoc -C --test -C last_arg -C --test -C loops -C --test -C pipeline -C --test -C positional -C --test -C read -C --test -C recvmsg -C --test -C redirect_scope -C --test -C sendmsg -C --test -C set_cmd -C --test -C set_opts -C --test -C shopt -C --test -C signalfd -C --test -C source -C --test -C strict -C --test -C test -C --test -C timeout -C --test -C times -C --test -C trap -C --test -C ulimit -C --test -C unset -C --test -C wait -C --test -C wordsplit -C --test -C xtrace -- -E 'test(/^intercept::tests::/) | binary(alias) | binary(cd_expand) | binary(colon) | binary(group) | binary(hash) | binary(help) | binary(heredoc) | binary(last_arg) | binary(loops) | binary(pipeline) | binary(positional) | binary(read) | binary(recvmsg) | binary(redirect_scope) | binary(sendmsg) | binary(set_cmd) | binary(set_opts) | binary(shopt) | binary(signalfd) | binary(source) | binary(strict) | binary(test) | binary(timeout) | binary(times) | binary(trap) | binary(ulimit) | binary(unset) | binary(wait) | binary(wordsplit) | binary(xtrace)'

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
    b"getopts",
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
    b"trap",
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
