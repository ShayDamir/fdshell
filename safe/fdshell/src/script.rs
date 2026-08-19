use crate::segment::Segment;
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
            Segment::Statement(stmt, off) => {
                let part = sub(text, off, stmt.len())?;
                if let Some(control) = crate::cond::run_cond_list(&part, cell)? {
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
                let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
                let full = sub(text, block_start + lead, raw.trim_ascii().len())?;
                if let Some(control) = crate::cond::run_cond_list(&full, cell)? {
                    return Ok(Some(control));
                }
            }
        }
    }
    Ok(None)
}

/// Subslice of a validated offset range; `None` is an internal invariant breach.
fn sub(text: &ScriptText, off: usize, len: usize) -> Result<ScriptText, Report<CmdError>> {
    let t = text.subslice(off, len).ok_or(CmdError::Never)?;
    Ok(t)
}
