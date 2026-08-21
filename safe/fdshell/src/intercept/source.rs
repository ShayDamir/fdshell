use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ImportedStr;
use sys::Origin;
use sys::Position;
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
    let substituted = crate::substitute::substitute_args(&cmdline.args, &cmdline.args_fq, cell)
        .change_context(CmdError::Resolve)?;
    let path = substituted.first().ok_or(CmdError::SourceNoFile)?;
    let extra = substituted.get(1..).unwrap_or(&[]);
    let saved = swap_positional(cell, extra, text)?;
    let result = run_sourced(path, cell);
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

/// Open `path`, read its content, and run it as a script in this shell.
fn run_sourced(
    path: &ShortCStr,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let fd = sys::openat2::open(path.export(), sys::fcntl::O_RDONLY)
        .change_context(CmdError::SourceOpen)?;
    let content = read_to_end(&fd)?;
    let data = ShortCStr::from_vec(content).change_context(CmdError::SourceNul)?;
    let script = ScriptText::new(data, Position::new(1, 1), Origin::File(path.clone()));
    crate::script::run_script(&script, cell)
}

/// Read `fd` to EOF.
fn read_to_end(fd: &sys::LocalFd) -> Result<Vec<u8>, Report<CmdError>> {
    let mut content = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = fd.read(&mut buf).change_context(CmdError::SourceRead)?;
        if n == 0 {
            break;
        }
        let slice = buf.get(..n).ok_or(CmdError::Never)?;
        content.extend_from_slice(slice);
    }
    Ok(content)
}

#[cfg(test)]
mod tests;
