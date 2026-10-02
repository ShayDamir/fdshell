//! Run a conditional part: subslice it (or reconstruct it with its heredoc
//! bodies) and hand it to `run_one`.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Run the conditional part spanning `line[start..i]` as a subsliced
/// statement. When the part carries heredoc bodies, the part text is
/// reconstructed as the command bytes plus a newline plus the bodies, so the
/// parse layer sees a contiguous heredoc.
pub(crate) fn run_part(
    text: &ScriptText,
    line: &[u8],
    start: usize,
    i: usize,
    bodies: &[Vec<u8>],
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let raw = line.get(start..i).unwrap_or(b"");
    if raw.trim_ascii().is_empty() {
        return Ok(None);
    }
    let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
    let part_start = start + lead;
    if bodies.is_empty() {
        let part_text = text
            .subslice(part_start, i - part_start)
            .ok_or(CmdError::Never)?;
        return crate::run::run_one(&part_text, cell);
    }
    let part_text = reconstruct(text, line, part_start, i, bodies)?;
    crate::run::run_one(&part_text, cell)
}

/// Reconstruct a part's text: the command bytes plus a newline plus the
/// bodies, as a fresh `ScriptText` at the part's start position.
fn reconstruct(
    text: &ScriptText,
    line: &[u8],
    part_start: usize,
    i: usize,
    bodies: &[Vec<u8>],
) -> Result<ScriptText, Report<CmdError>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(line.get(part_start..i).unwrap_or(b""));
    bytes.push(b'\n');
    for body in bodies {
        bytes.extend_from_slice(body);
    }
    let view = text.subslice(part_start, 1).ok_or(CmdError::Never)?;
    let data = sys::ShortCStr::from_vec(bytes).change_context(CmdError::Never)?;
    Ok(ScriptText::new(data, view.start, view.origin.clone()))
}

#[cfg(test)]
mod tests;
