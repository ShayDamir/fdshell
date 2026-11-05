//! The two executors behind `run_script`'s dispatch, one per segment shape: a
//! statement — with the joined text `script/merge.rs` built for it when a
//! here-doc region split its list (task #492) — and a closed keyword block.
//! Both run their unit through `run_cond_list` and report the loop control they
//! caught; the dispatch in `script.rs` owns the scan, the pre-passes and the
//! early return.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::script::merge::{Unit, text_at};
use crate::script::unclosed::report;
use crate::segment::Segment;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Run one statement unit: the joined statement `merge::units` built for it, or
/// the segment as it stands.
pub(super) fn run_statement(
    unit: &Unit<'_>,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let Segment::Statement {
        cmd,
        off,
        bodies,
        background,
        // The mid-run flag is the REPL's continuation signal, not a
        // bail: the parse layer produces the pinned missing-closer
        // diagnostic and its rc (tasks #232/#236), so the script
        // runner runs the buffer as written (task #294).
        midrun_open: _,
        // The join flag and the join points are read by `merge::units`
        // before the loop; an unmerged statement runs the text as written.
        join_next: _,
        joins: _,
    } = unit.segment
    else {
        // Unreachable: the dispatch in `script.rs` routes only statement
        // units here.
        return Ok(None);
    };
    // A statement whose list a here-doc region split runs as the
    // joined statement `script/merge.rs` built for it.
    let (bytes, bodies, background) = match &unit.joined {
        Some(joined) => (joined.text.as_slice(), &joined.bodies, joined.background),
        None => (*cmd, bodies, *background),
    };
    crate::verbose::trace(bytes, cell);
    let part = match &unit.joined {
        Some(joined) => text_at(&joined.text, *off, text)?,
        None => sub(text, *off, bytes.len())?,
    };
    if let Some(control) = crate::cond::run_cond_list(&part, bodies, cell, false, background)? {
        return Ok(Some(control));
    }
    Ok(None)
}

/// Run one closed keyword block: its **command** bytes only, its body bytes
/// travelling as `run_cond_list`'s bodies exactly like a statement's.
pub(super) fn run_block(
    line: &[u8],
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
    block_start: &usize,
    end_pos: &usize,
    closed: &bool,
    bodies: &[Vec<u8>],
) -> Result<Option<LoopControl>, Report<CmdError>> {
    if !closed {
        // Named, not bare: `'if' is missing its closing 'fi'` with
        // the caret under the opener (task #277, STYLE §4.7).
        return Err(report(line, *block_start));
    }
    let raw = line.get(*block_start..*end_pos).unwrap_or(b"");
    crate::verbose::trace(raw.trim_ascii(), cell);
    let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
    let cmd_start = *block_start + lead;
    // The block's text is its **command** bytes only, and its body
    // bytes travel as `run_cond_list`'s bodies exactly like a
    // statement's (task #234): `cond::parts::take_bodies` gives the
    // part that owns the `<<` operator its own body, so a split
    // past the closing keyword never leaves a heredoc part bodyless.
    // An extension-mode block (a condition's bodies are inside the
    // span) claims no bodies here and keeps its verbatim text.
    let full = sub(text, cmd_start, raw.trim_ascii().len())?;
    // A block statement's own `&` runs it foreground (measured
    // divergence, task #326 — compound background is a follow-up).
    if let Some(control) = crate::cond::run_cond_list(&full, bodies, cell, false, false)? {
        return Ok(Some(control));
    }
    Ok(None)
}

/// Subslice of a validated offset range; `None` is an internal invariant breach.
fn sub(text: &ScriptText, off: usize, len: usize) -> Result<ScriptText, Report<CmdError>> {
    let t = text.subslice(off, len).ok_or(CmdError::Never)?;
    Ok(t)
}
