//! `flock %var [--shared] [--wait] [--nowait] [--unlock]` — advisory lock on
//! an existing `%var` fd.
//!
//! The lock is taken on the fd var's open file description, so it outlives
//! this builtin's child and is held by the shell until `--unlock` or the var
//! is closed. Coordinates processes by handle, never by path. Defaults to an
//! exclusive blocking lock.

// mutants: -C --lib -C --test -C flock -- -E 'test(/^child::flock::/) | binary(flock)'

mod parse;

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};

use super::Ctx;

pub(super) fn handle_flock(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::flock_parse(ctx.refs, ctx.args)?;
    let fd = ctx
        .state
        .fd_var(&cfg.var)
        .ok_or(BuiltinError::FdVarNotFound)?;
    fd.fd
        .flock(cfg.operation)
        .change_context(BuiltinError::Syscall)?;
    Ok(0)
}

#[cfg(test)]
mod tests;
