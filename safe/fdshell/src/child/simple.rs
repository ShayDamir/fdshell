use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};

use super::Ctx;

pub(super) fn handle_true(_: &Ctx) -> Result<i32, Report<BuiltinError>> {
    Ok(0)
}

pub(super) fn handle_false(_: &Ctx) -> Result<i32, Report<BuiltinError>> {
    Ok(1)
}

pub(super) fn handle_pwd(_: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cwd = sys::env::getcwd().change_context(BuiltinError::Io)?;
    sys::OUT.write_all(&cwd).change_context(BuiltinError::Io)?;
    sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(0)
}

pub(super) fn handle_help(_: &Ctx) -> Result<i32, Report<BuiltinError>> {
    crate::child::help::print_help()
}

pub(super) fn handle_echo(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    for (i, arg) in ctx.refs.iter().enumerate() {
        if i > 0 {
            sys::OUT.write_all(b" ").change_context(BuiltinError::Io)?;
        }
        sys::OUT
            .write_all(arg.to_bytes())
            .change_context(BuiltinError::Io)?;
    }
    sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(0)
}
