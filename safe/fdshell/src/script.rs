use crate::segment::Segment;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use sys::ScriptText;
use sys::fork_cell::ForkCell;

pub(crate) fn run_script(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    for segment in crate::segment::scan_segments(line, false) {
        match segment {
            Segment::Statement { cmd, off, bodies } => {
                crate::verbose::trace(line.get(off..off + cmd.len()).unwrap_or(b""), cell);
                let part = sub(text, off, cmd.len())?;
                let body_bytes = extract_bodies(line, &bodies);
                if let Some(control) = crate::cond::run_cond_list(&part, &body_bytes, cell, false)?
                {
                    return Ok(Some(control));
                }
            }
            Segment::Block {
                block_start,
                end_pos,
                closed,
            } => {
                ensure!(closed, CmdError::Parse);
                let raw = line.get(block_start..end_pos).unwrap_or(b"");
                crate::verbose::trace(raw.trim_ascii(), cell);
                let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
                let full = sub(text, block_start + lead, raw.trim_ascii().len())?;
                if let Some(control) = crate::cond::run_cond_list(&full, &[], cell, false)? {
                    return Ok(Some(control));
                }
            }
        }
    }
    Ok(None)
}

/// Extract the body bytes of a statement's heredoc regions.
fn extract_bodies(line: &[u8], bodies: &[(usize, usize)]) -> Vec<Vec<u8>> {
    bodies
        .iter()
        .map(|&(s, e)| line.get(s..e).unwrap_or(b"").to_vec())
        .collect()
}

/// Subslice of a validated offset range; `None` is an internal invariant breach.
fn sub(text: &ScriptText, off: usize, len: usize) -> Result<ScriptText, Report<CmdError>> {
    let t = text.subslice(off, len).ok_or(CmdError::Never)?;
    Ok(t)
}
