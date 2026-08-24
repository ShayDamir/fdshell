mod args;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// `alias [name[=value]…]` — define and/or display aliases.
pub(crate) fn run_alias(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    _text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "alias", cmdline)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    if cmdline.args.is_empty() {
        list_aliases(&state);
    } else {
        let mut out = Vec::new();
        for arg in cmdline.args.iter() {
            args::apply_alias_arg(&mut state, arg, &mut out)?;
        }
        sys::OUT.write_all(&out).ok();
    }
    state.set_last_exit(0);
    Ok(true)
}

/// `unalias name…` — remove aliases.
pub(crate) fn run_unalias(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    _text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "unalias", cmdline)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    for name in cmdline.args.iter() {
        if state.aliases.remove(name).is_none() {
            bail!(CmdError::AliasNotFound { name: name.clone() });
        }
    }
    state.set_last_exit(0);
    Ok(true)
}

fn list_aliases(state: &ShellState) {
    let mut names: Vec<&ShortCStr> = state.aliases.keys().collect();
    names.sort_unstable_by(|a, b| a.as_bytes().unwrap_or(&[]).cmp(b.as_bytes().unwrap_or(&[])));
    let mut out = Vec::new();
    for name in names {
        if let Some(value) = state.aliases.get(name) {
            push_definition(name, value, &mut out);
        }
    }
    sys::OUT.write_all(&out).ok();
}

pub(super) fn push_definition(name: &ShortCStr, value: &ShortCStr, out: &mut Vec<u8>) {
    out.extend_from_slice(b"alias ");
    out.extend_from_slice(name.as_bytes().unwrap_or(&[]));
    out.extend_from_slice(b"='");
    for &b in value.as_bytes().unwrap_or(&[]) {
        if b == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(b);
        }
    }
    out.extend_from_slice(b"'\n");
}
