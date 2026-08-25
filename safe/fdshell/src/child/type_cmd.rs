//! `type NAME...` — how the shell resolves each name (bash compat).
//!
//! Order mirrors resolution: alias, function, keyword, builtin, fd variable,
//! then PATH. A name found nowhere reports `type: NAME: not found` and the
//! exit status is 1.

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

const KEYWORDS: &[&[u8]] = &[
    b"break",
    b"case",
    b"continue",
    b"do",
    b"done",
    b"else",
    b"elif",
    b"esac",
    b"fi",
    b"for",
    b"if",
    b"return",
    b"then",
    b"until",
    b"while",
];

pub(super) fn handle_type(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    if refs.is_empty() {
        bail!(BuiltinError::MissingArgument("name"));
    }
    let mut found = true;
    for name in refs {
        found = describe(name, state)? && found;
    }
    Ok((!found) as i32)
}

fn describe(name: &CStr, state: &ShellState) -> Result<bool, Report<BuiltinError>> {
    let key = ShortCStr::from_vec(name.to_bytes().to_vec()).change_context(BuiltinError::Never)?;
    if let Some(value) = state.aliases.get(&key) {
        let vb = value.as_bytes().change_context(BuiltinError::Never)?;
        return emit(&[name.to_bytes(), b" is aliased to '", vb, b"'"]).map(|_| true);
    }
    if state.functions.contains_key(&key) {
        return emit(&[name.to_bytes(), b" is a shell function"]).map(|_| true);
    }
    if KEYWORDS.iter().any(|kw| *kw == name.to_bytes()) {
        return emit(&[name.to_bytes(), b" is a shell keyword"]).map(|_| true);
    }
    if super::dispatch::is_dispatched(&key) {
        return emit(&[name.to_bytes(), b" is a shell builtin"]).map(|_| true);
    }
    if state.fds.contains_key(&key) || state.arrays.contains_key(&key) {
        return emit(&[name.to_bytes(), b" is an fd variable"]).map(|_| true);
    }
    if let Ok(path) = crate::exec::resolve_path_str(&key) {
        let pb = path.as_bytes().change_context(BuiltinError::Never)?;
        return emit(&[name.to_bytes(), b" is ", pb]).map(|_| true);
    }
    let _ = sys::ERR.write_all(b"type: ");
    let _ = sys::ERR.write_all(name.to_bytes());
    let _ = sys::ERR.write_all(b": not found\n");
    Ok(false)
}

fn emit(parts: &[&[u8]]) -> Result<(), Report<BuiltinError>> {
    for p in parts {
        sys::OUT.write_all(p).change_context(BuiltinError::Io)?;
    }
    sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests;
