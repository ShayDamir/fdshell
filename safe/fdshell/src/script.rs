use crate::segment::Segment;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::error::parse::ParseError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use sys::ScriptText;
use sys::fork_cell::ForkCell;

mod merge;
mod segments;
mod unclosed;

pub(crate) fn run_script(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let segments = crate::segment::scan_segments(line, false);
    // The bash preflight: an invalid plain-`&` split is a parse error that
    // runs nothing, so the whole scan is checked before any segment executes.
    if segments
        .iter()
        .any(|seg| matches!(seg, Segment::AsyncInvalid))
    {
        // `bail!` cannot carry the `.change_context()` this cross-crate
        // conversion needs (STYLE §4.14): construct the report explicitly.
        return Err(
            Report::new(ParseError::UnexpectedChar { ch: b'&' }).change_context(CmdError::Parse)
        );
    }
    // The same preflight for the stray `;;` (task #423): bash stops before
    // running anything, and the scan marks the line, so the report is a parse
    // error. It rides **after** the async check — a line carrying both keeps
    // the async message (the order is pinned by `tests/async.rs`).
    if segments
        .iter()
        .any(|seg| matches!(seg, Segment::UnexpectedSemiSemi))
    {
        return Err(Report::new(ParseError::UnexpectedSemiSemi).change_context(CmdError::Parse));
    }
    for unit in merge::units(&segments) {
        match unit.segment {
            Segment::Statement { .. } => {
                if let Some(control) = segments::run_statement(&unit, text, cell)? {
                    return Ok(Some(control));
                }
            }
            Segment::Block {
                block_start,
                end_pos,
                closed,
                // The group span's tail-closedness flag is the REPL's signal
                // (task #401); the parse layer owns the runner's report, exactly
                // as the `Statement` arm's flag (tasks #232/#236/#294).
                midrun_open: _,
                bodies,
            } => {
                if let Some(control) =
                    segments::run_block(line, text, cell, block_start, end_pos, closed, bodies)?
                {
                    return Ok(Some(control));
                }
            }
            // Unreachable: the pre-pass above bails on every invalid split and
            // every stray `;;` (task #423).
            Segment::AsyncInvalid | Segment::UnexpectedSemiSemi => {}
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
