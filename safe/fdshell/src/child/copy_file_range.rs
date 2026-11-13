//! `copy_file_range %in %out [COUNT]` — zero-copy copy of bytes between two
//! `%var` fds using `copy_file_range(2)`. Prints the number of bytes copied.

// mutants: -C --lib -C --test -C copy_file_range -- -E 'test(/^child::copy_file_range::/) | binary(copy_file_range)'

use builtins::copy_file_range::{copy_file_range_exec, parse::copy_file_range_parse};
use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};

use super::Ctx;

pub(super) fn handle_copy_file_range(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = copy_file_range_parse(ctx.refs, ctx.args)?;
    let in_fd = ctx
        .state
        .fd_var(&cfg.in_var)
        .ok_or(BuiltinError::FdVarNotFound)?;
    let out_fd = ctx
        .state
        .fd_var(&cfg.out_var)
        .ok_or(BuiltinError::FdVarNotFound)?;
    let copied = copy_file_range_exec(&cfg, &in_fd.fd, &out_fd.fd)?;
    let line = sys::format!("{copied}\n").change_context(BuiltinError::Io)?;
    sys::OUT.write_str(&line).change_context(BuiltinError::Io)?;
    Ok(0)
}

#[cfg(test)]
mod tests;
