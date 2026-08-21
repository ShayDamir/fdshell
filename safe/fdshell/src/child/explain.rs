use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};
use sys::Origin;
use sys::ShortCStr;

/// `explain NAME` / `explain N` — print the provenance of a variable or
/// positional parameter. Read-only; unset names report `unset` with exit 0.
pub(super) fn handle_explain(
    _: ShortCStr,
    _refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    if args.len() > 1 {
        bail!(BuiltinError::InvalidArgument("name"));
    }
    let name = args
        .first()
        .ok_or(BuiltinError::MissingArgument("var or index"))?;
    let bytes = name
        .as_bytes()
        .change_context(BuiltinError::InvalidArgument("name"))?;
    let (display, traced) = if is_all_digits(bytes) {
        let idx = parse_index(bytes).ok_or(BuiltinError::InvalidArgument("index"))?;
        let d = sys::format!("${idx}").change_context(BuiltinError::Io)?;
        (d, state.positional.get(idx))
    } else {
        (name.clone(), state.strings.get(name))
    };
    match traced {
        Some(v) => write_trace(&display, v),
        None => write_unset(&display),
    }?;
    Ok(0)
}

fn write_trace(display: &ShortCStr, v: &sys::ImportedStr) -> Result<(), Report<BuiltinError>> {
    let origin = origin_phrase(&v.trace.origin)?;
    let msg = match v.trace.set_at {
        Some(p) => sys::format!(
            "{display}=\"{}\" (set on line {}, column {}, from {origin})",
            v.value,
            p.line,
            p.column
        )
        .change_context(BuiltinError::Io)?,
        None => sys::format!("{display}=\"{}\" (from {origin})", v.value)
            .change_context(BuiltinError::Io)?,
    };
    sys::OUT.write_str(&msg).change_context(BuiltinError::Io)?;
    sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(())
}

pub(super) fn write_unset(display: &ShortCStr) -> Result<(), Report<BuiltinError>> {
    let msg = sys::format!("{display} is unset").change_context(BuiltinError::Io)?;
    sys::OUT.write_str(&msg).change_context(BuiltinError::Io)?;
    sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(())
}

pub(super) fn origin_phrase(origin: &Origin) -> Result<ShortCStr, Report<BuiltinError>> {
    let s = match origin {
        Origin::CliArgument(i) => sys::format!("argv[{i}]"),
        Origin::EnvVar(n) => sys::format!("environment variable {n}"),
        Origin::File(p) => sys::format!("file {p}"),
        Origin::Stdin => sys::format!("stdin"),
        Origin::CommandOutput => sys::format!("command output"),
        Origin::Read(n) => sys::format!("fd {n}"),
        Origin::Shell => sys::format!("shell default"),
        Origin::Captured(n) => sys::format!("tag {n}"),
    };
    s.change_context(BuiltinError::Io)
}

fn is_all_digits(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.iter().all(|b| b.is_ascii_digit())
}

fn parse_index(bytes: &[u8]) -> Option<usize> {
    bytes.iter().try_fold(0usize, |acc, &d| {
        acc.checked_mul(10)
            .and_then(|a| a.checked_add((d - b'0') as usize))
    })
}
