mod file;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use alloc::collections::VecDeque;
use error_stack::{Report, ResultExt};
use sys::ImportedStr;
use sys::ScriptText;
use sys::ShortCStr;
use sys::Trace;
use sys::fork_cell::ForkCell;

/// `source` / `.`: read a file and run its content as a script in this shell.
///
/// Extra arguments replace the positional parameters (as `set --` does) for
/// the duration of the sourced script; the previous ones are restored after.
pub(crate) fn run_source(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    super::validation::validate_intercept(line, "source", cmdline)?;
    let substituted = crate::substitute::substitute_args(
        &cmdline.args,
        &cmdline.args_mask,
        &cmdline.args_quoted,
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let path = substituted.first().ok_or(CmdError::SourceNoFile)?;
    let extra = substituted.get(1..).unwrap_or(&[]);
    let saved = swap_positional(cell, extra, text)?;
    let result = super::last_arg_frame::with_eval_frame(cell, || file::run_sourced(path, cell));
    if let Some(saved) = saved {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_positional(saved);
    }
    result
}

/// Replace the positional parameters with `extra` and return the saved ones.
/// Returns `None` when `extra` is empty (positional parameters stay as-is).
fn swap_positional(
    cell: &ForkCell<ShellState>,
    extra: &[ShortCStr],
    text: &ScriptText,
) -> Result<Option<VecDeque<ImportedStr>>, Report<CmdError>> {
    if extra.is_empty() {
        return Ok(None);
    }
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let saved = core::mem::take(&mut state.positional);
    let positional = extra
        .iter()
        .map(|s| ImportedStr::new(s.clone(), Trace::at(text.start, text.origin.clone())))
        .collect();
    state.set_positional(positional);
    Ok(Some(saved))
}

#[cfg(test)]
mod tests;
