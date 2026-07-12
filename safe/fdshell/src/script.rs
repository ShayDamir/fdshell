use crate::loop_control::LoopControl;
use crate::segment::Segment;
use error_stack::{Report, ensure};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_script(
    line: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let segments = crate::segment::scan_segments(line, false);
    for segment in segments {
        match segment {
            Segment::Statement(stmt) => {
                if let Some(control) = crate::cond::run_cond_list(stmt, cell)? {
                    return Ok(Some(control));
                }
            }
            Segment::Block {
                block_start,
                end_pos,
                closed,
            } => {
                ensure!(closed, CmdError::Parse);
                let full = line.get(block_start..end_pos).unwrap_or(b"").trim_ascii();
                if let Some(control) = crate::cond::run_cond_list(full, cell)? {
                    return Ok(Some(control));
                }
            }
        }
    }
    Ok(None)
}
