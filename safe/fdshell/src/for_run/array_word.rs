use alloc::vec::Vec;

use crate::error::cmd::CmdError;
use crate::state::{FdVar, ShellState};
use error_stack::{Report, ResultExt};
use sys::ImportedStr;
use sys::fork_cell::ForkCell;

/// Dup every entry of the array referenced by `word`; `None` for non-arrays.
pub(super) fn dup_array_word(
    word: &ImportedStr,
    cell: &ForkCell<ShellState>,
) -> Result<Option<Vec<FdVar>>, Report<CmdError>> {
    let Some(name) = word.value.strip_prefix(b"%") else {
        return Ok(None);
    };
    let state = cell.borrow().change_context(CmdError::Never)?;
    let Some(arr) = state.arrays.get(&name) else {
        return Ok(None);
    };
    let mut dups = Vec::with_capacity(arr.len());
    for entry in arr {
        let fd = entry.fd.try_clone().change_context(CmdError::Fd)?;
        dups.push(FdVar {
            fd,
            trace: entry.trace.clone(),
        });
    }
    Ok(Some(dups))
}
