use crate::state::ShellState;
use builtins::error::BuiltinError;
use error_stack::Report;
use sys::LocalFd;

use super::Ctx;

fn sock(state: &ShellState) -> Result<&LocalFd, Report<BuiltinError>> {
    Ok(state
        .shell_sock
        .as_ref()
        .ok_or(BuiltinError::SendFdFailed)?)
}

pub(super) fn handle_fchmod(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = builtins::fchmod::parse::fchmod_parse(ctx.refs)?;
    builtins::fchmod::fchmod_exec(&cfg).map(|()| 0)
}

pub(super) fn handle_pipe(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let sock = sock(ctx.state)?;
    let cfg = builtins::pipe::parse::pipe_parse(ctx.refs)?;
    builtins::pipe::pipe_exec(cfg.flags, sock).map(|()| 0)
}

pub(super) fn handle_mkdirat(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let sock = sock(ctx.state)?;
    let cfg = builtins::mkdirat::parse::mkdirat_parse(ctx.refs)?;
    builtins::mkdirat::mkdirat_exec(&cfg, sock).map(|()| 0)
}

pub(super) fn handle_memfd(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let sock = sock(ctx.state)?;
    let cfg = builtins::memfd::parse::memfd_parse(ctx.refs)?;
    builtins::memfd::memfd_exec(&cfg, sock).map(|()| 0)
}

pub(super) fn handle_openat2(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let sock = sock(ctx.state)?;
    let cfg = builtins::openat2::parse::openat2_parse(ctx.refs)?;
    builtins::openat2::openat2_exec(&cfg, sock).map(|()| 0)
}

pub(super) fn handle_timerfd(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let sock = sock(ctx.state)?;
    let cfg = builtins::timerfd::parse::timerfd_parse(ctx.refs)?;
    builtins::timerfd::timerfd_exec(&cfg, sock).map(|()| 0)
}

pub(super) fn handle_eventfd(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let sock = sock(ctx.state)?;
    let cfg = builtins::eventfd::parse::eventfd_parse(ctx.refs)?;
    builtins::eventfd::eventfd_exec(&cfg, sock).map(|()| 0)
}

pub(super) fn handle_renameat2(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = builtins::renameat2::parse::renameat2_parse(ctx.refs)?;
    builtins::renameat2::renameat2_exec(&cfg).map(|()| 0)
}

#[cfg(test)]
mod tests;
