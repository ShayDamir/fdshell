use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

use super::Ctx;
use super::explain::{origin_phrase, write_unset};

/// `fdexplain %NAME` — print the provenance of an fd variable.
/// Read-only; unset names report `unset` with exit 0.
pub(super) fn handle_fdexplain(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    if ctx.args.len() > 1 {
        bail!(BuiltinError::InvalidArgument("name"));
    }
    let raw = ctx
        .args
        .first()
        .ok_or(BuiltinError::MissingArgument("fd var"))?;
    let name = raw.strip_prefix(b"%").unwrap_or_else(|| raw.clone());
    match ctx.state.fds.get(&name) {
        Some(v) => write_trace(raw, &v.trace),
        None => write_unset(raw),
    }?;
    Ok(0)
}

fn write_trace(display: &ShortCStr, trace: &sys::Trace) -> Result<(), Report<BuiltinError>> {
    let origin = origin_phrase(&trace.origin)?;
    let msg = match trace.set_at {
        Some(p) => sys::format!(
            "{display} (set on line {}, column {}, from {origin})",
            p.line,
            p.column
        )
        .change_context(BuiltinError::Io)?,
        None => sys::format!("{display} (from {origin})").change_context(BuiltinError::Io)?,
    };
    sys::OUT.write_str(&msg).change_context(BuiltinError::Io)?;
    sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(())
}
