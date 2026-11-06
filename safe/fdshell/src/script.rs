use crate::segment::Segment;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::error::parse::{ParseError, report_stray_closer};
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
    // The same preflight for a **stray closer** word (task #436): bash's grammar
    // refuses a closing keyword word that closes no open block of its kind, and
    // because bash parses the whole input before running anything, nothing on the
    // line runs. The scan marked the word's offset, so the report draws the caret
    // there (the `report_unbalanced_quote` mechanism).
    //
    // It rides **after** the async and `;;` arms — the order is a decision, pinned
    // by the two-shape rows of `tests/block_closers.rs::a_stray_closer_reports_at_preflight`.
    if let Some(&Segment::StrayCloser { off }) = segments
        .iter()
        .find(|seg| matches!(seg, Segment::StrayCloser { .. }))
    {
        return Err(report_stray_closer(line, off).change_context(CmdError::Parse));
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
            // Unreachable: the pre-pass above bails on every invalid split, every
            // stray `;;` and every stray closer (tasks #423, #326, #436).
            Segment::AsyncInvalid | Segment::UnexpectedSemiSemi | Segment::StrayCloser { .. } => {}
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
