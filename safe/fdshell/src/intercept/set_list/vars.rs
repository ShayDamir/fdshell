//! Bare `set`: positional parameters (raw values) followed by `NAME=value`
//! lines for all string variables and exports, sorted by name.

use alloc::vec::Vec;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// Positional parameters (raw values) followed by `NAME=value` lines for all
/// string variables and exports, sorted by name.
pub fn list_vars(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let mut out = Vec::new();
    for p in &state.positional {
        out.extend_from_slice(p.value.as_bytes().change_context(CmdError::Resolve)?);
        out.push(b'\n');
    }
    let mut names: Vec<&ShortCStr> = state.strings.keys().collect();
    let ifs: ShortCStr = c"IFS".into();
    // `IFS` lives in `state.ifs` until first assigned; always list it (bash).
    if !names.iter().any(|n| n.eq_bytes(b"IFS")) {
        names.push(&ifs);
    }
    for name in state.exports.keys() {
        let bytes = name.as_bytes().change_context(CmdError::Resolve)?;
        if !names.iter().any(|n| n.eq_bytes(bytes)) {
            names.push(name);
        }
    }
    super::sort_by_bytes(&mut names);
    for name in names {
        let value = if name.eq_bytes(b"IFS") {
            state.ifs.clone()
        } else {
            state
                .strings
                .get(name)
                .or_else(|| state.exports.get(name))
                .ok_or(CmdError::Never)?
                .value
                .clone()
        };
        out.extend_from_slice(name.as_bytes().change_context(CmdError::Resolve)?);
        out.push(b'=');
        out.extend_from_slice(value.as_bytes().change_context(CmdError::Resolve)?);
        out.push(b'\n');
    }
    sys::OUT.write_all(&out).ok();
    state.set_last_exit(0);
    Ok(())
}
