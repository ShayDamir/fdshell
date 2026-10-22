use crate::segment::Segment;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::error::parse::ParseError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use sys::ScriptText;
use sys::fork_cell::ForkCell;

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
    for segment in segments {
        match segment {
            Segment::Statement {
                cmd,
                off,
                bodies,
                background,
                // The mid-run flag is the REPL's continuation signal, not a
                // bail: the parse layer produces the pinned missing-closer
                // diagnostic and its rc (tasks #232/#236), so the script
                // runner runs the buffer as written (task #294).
                midrun_open: _,
            } => {
                crate::verbose::trace(line.get(off..off + cmd.len()).unwrap_or(b""), cell);
                let part = sub(text, off, cmd.len())?;
                if let Some(control) =
                    crate::cond::run_cond_list(&part, &bodies, cell, false, background)?
                {
                    return Ok(Some(control));
                }
            }
            Segment::Block {
                block_start,
                end_pos,
                closed,
                bodies,
            } => {
                if !closed {
                    // Named, not bare: `'if' is missing its closing 'fi'` with
                    // the caret under the opener (task #277, STYLE §4.7).
                    return Err(unclosed::report(line, block_start));
                }
                let raw = line.get(block_start..end_pos).unwrap_or(b"");
                crate::verbose::trace(raw.trim_ascii(), cell);
                let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
                let cmd_start = block_start + lead;
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
                if let Some(control) =
                    crate::cond::run_cond_list(&full, &bodies, cell, false, false)?
                {
                    return Ok(Some(control));
                }
            }
            // Unreachable: the pre-pass above bails on every invalid split.
            Segment::AsyncInvalid => {}
        }
    }
    Ok(None)
}

/// Subslice of a validated offset range; `None` is an internal invariant breach.
fn sub(text: &ScriptText, off: usize, len: usize) -> Result<ScriptText, Report<CmdError>> {
    let t = text.subslice(off, len).ok_or(CmdError::Never)?;
    Ok(t)
}

#[cfg(test)]
mod tests;
