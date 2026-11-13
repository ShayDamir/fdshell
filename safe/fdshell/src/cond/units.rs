//! The cond layer learns its separators through the **one** scanner (task
//! #557): `run_cond_list`'s walk splits on operators only, and a condition
//! that lists several commands is bash's `compound_list` — every command runs,
//! in order, and the **last** one's status reaches the test. The splitter is
//! `crate::segment::scan_segments`, never a re-derivation: quotes, comments,
//! here-doc regions, group braces, keyword blocks, the glued `;;`, the
//! line-leading `;`, the plain-`&` split and POSIX #2.9's AND-IF-TERMINATOR
//! join (task #492 — a newline **behind** a top-level `&&`/`||` keeps the run
//! one unit) are read by that scan alone. A text that yields one unit — every
//! pinned single-unit condition and top-level statement — runs the operator
//! walk exactly as before this module existed. The scan's marks
//! (LeadingSeparator, AsyncInvalid, UnexpectedSemiSemi, StrayCloser) stay the
//! pre-passes' business, run over the whole text by the entry before anything
//! runs; this dispatcher acts only on statement and block units and invents no
//! error paths. Loop control raised by **any** unit leaves the condition at
//! once, as the existing early-return contract does; the last unit's status is
//! what the state carries to the test, so no extra bookkeeping is needed. The
//! plain-`&` flag rides on the **last** unit only — a plain `&` ends the
//! statement before it (task #326), and a condition slice carries no `&` byte
//! (task #356): the flag is the parse layer's, handed to the entry as an
//! argument.

// mutants: -C --lib -C --test -C cond_blocks -C --test -C errexit -C --test -C set_opts -C --test -C group -C --test -C loops -- -E 'test(/^cond::/) | test(/^tests::run::/) | binary(errexit) | binary(cond_blocks) | binary(set_opts) | binary(group) | binary(loops)'

use crate::cond::{parts, run_list};
use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::segment::Segment;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Run the (pre-passed) cond text: one unit → the walk as before; several →
/// every unit in order, each through `run_list` with the bodies it owns, the
/// background flag on the last one only.
pub(super) fn run_units(
    text: &ScriptText,
    bodies: &[Vec<u8>],
    cell: &ForkCell<ShellState>,
    background: bool,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let units = units_of(line);
    if units.len() < 2 {
        return run_list(text, bodies, cell, background);
    }
    let mut body_idx = 0;
    for (n, seg) in units.iter().enumerate() {
        let last = n + 1 == units.len();
        let own = own_bodies(seg, line, bodies, &mut body_idx);
        let unit_text = unit_text(text, line, seg)?;
        if let Some(control) = run_list(&unit_text, &own, cell, background && last)? {
            return Ok(Some(control));
        }
    }
    Ok(None)
}

/// The scanner's actionable units in scan order; the marks are the pre-passes'
/// business and are never acted on here.
fn units_of(line: &[u8]) -> Vec<Segment<'_>> {
    crate::segment::scan_segments(line, false)
        .into_iter()
        .filter(|seg| matches!(seg, Segment::Statement { .. } | Segment::Block { .. }))
        .collect()
}

/// The unit's own bodies: the ones the scanner assigned it, else the unit's
/// share of the caller's list — a single-physical-line scan assigns none (the
/// region bytes live on the lines below and travel in the caller's list), and
/// the share is taken with the same per-part count `cond/parts.rs` uses within
/// one list, so the global operator order is preserved across units run in
/// order.
fn own_bodies(
    seg: &Segment<'_>,
    line: &[u8],
    bodies: &[Vec<u8>],
    body_idx: &mut usize,
) -> Vec<Vec<u8>> {
    let own = match seg {
        Segment::Statement { bodies, .. } | Segment::Block { bodies, .. } => bodies,
        // Unreachable: the dispatcher filters the scan's marks out.
        _ => return Vec::new(),
    };
    if !own.is_empty() {
        return own.clone();
    }
    let (start, len) = span(seg);
    parts::take_bodies(line, start, start + len, bodies, body_idx)
}

/// The unit's text: a statement's run bytes, a block's **command** bytes as
/// `script/segments.rs::run_block` dispatches them (the unclosed-closer report
/// stays the parse layer's). A run's span is already trimmed; the block's is
/// trimmed here, and for an already-trimmed span the trim is the identity.
fn unit_text(
    text: &ScriptText,
    line: &[u8],
    seg: &Segment<'_>,
) -> Result<ScriptText, Report<CmdError>> {
    let (start, len) = span(seg);
    let raw = line.get(start..start + len).unwrap_or(b"");
    let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
    let t = text
        .subslice(start + lead, raw.trim_ascii().len())
        .ok_or(CmdError::Never)?;
    Ok(t)
}

/// The unit's span in the scanned text: a statement's run (offset, length), a
/// block's whole span (start, length).
fn span(seg: &Segment<'_>) -> (usize, usize) {
    match seg {
        Segment::Statement { cmd, off, .. } => (*off, cmd.len()),
        Segment::Block {
            block_start,
            end_pos,
            ..
        } => (*block_start, end_pos.saturating_sub(*block_start)),
        // Unreachable: the dispatcher filters the scan's marks out.
        _ => (0, 0),
    }
}

#[cfg(test)]
mod tests;
