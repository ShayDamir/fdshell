mod boundary;
mod errexit;
mod part;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::scan::{ScanState, heredoc};
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// `exempt` disables errexit on the list's final segment (the conditions of
/// `if`/`while`/`until` are exempt, as in bash).
pub(crate) fn run_cond_list(
    text: &ScriptText,
    bodies: &[Vec<u8>],
    cell: &ForkCell<ShellState>,
    exempt: bool,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let mut state = ScanState::new();
    let mut start = 0;
    let mut body_idx = 0;
    loop {
        let (i, is_or) = boundary::next_boundary(line, start, &mut state);
        let part_bodies = take_bodies(line, start, i, bodies, &mut body_idx);
        if i == line.len() {
            if let Some(control) = part::run_part(text, line, start, i, &part_bodies, cell)? {
                return Ok(Some(control));
            }
            // Errexit only for a final part that actually ran: a list that
            // ended on a skipped `&&` chain keeps the failure in the exempt
            // preceding part and must not stop the shell.
            if !exempt && !is_empty_part(line, start, i) && errexit::should_exit(cell)? {
                return Ok(Some(LoopControl::Exit));
            }
            break;
        }
        if !is_empty_part(line, start, i) {
            if let Some(control) = part::run_part(text, line, start, i, &part_bodies, cell)? {
                return Ok(Some(control));
            }
            let st = cell.borrow().change_context(CmdError::Never)?;
            let ok = st.last_status.exit_code() == 0;
            if is_or && ok {
                return Ok(None);
            }
            if !is_or && !ok {
                // `&&` after a failure: skip the rest of the list up to `||`.
                start = skip_to_or(line, i + 2, &mut state, bodies, &mut body_idx);
                continue;
            }
        }
        start = i + 2;
    }
    Ok(None)
}

/// After a failing `&&` part, the next `||` (or end of line): everything up
/// to it is skipped without running. The skipped parts' body bytes are still
/// consumed (the bodies are in global operator order).
fn skip_to_or(
    line: &[u8],
    from: usize,
    state: &mut ScanState,
    bodies: &[Vec<u8>],
    body_idx: &mut usize,
) -> usize {
    let mut pos = from;
    loop {
        let (p, is_or) = boundary::next_boundary(line, pos, state);
        take_bodies(line, pos, p, bodies, body_idx);
        if is_or || p == line.len() {
            return p;
        }
        pos = p + 2;
    }
}

/// The next `n` body bytes for the part `line[start..end]`, where `n` is the
/// part's byte-level `<<` operator count. The bodies are in global operator
/// order, so the part's share is the next `n` entries.
fn take_bodies(
    line: &[u8],
    start: usize,
    end: usize,
    bodies: &[Vec<u8>],
    body_idx: &mut usize,
) -> Vec<Vec<u8>> {
    let n = heredoc::operator_count(line, start, end);
    let mut out = Vec::new();
    for _ in 0..n {
        if let Some(body) = bodies.get(*body_idx) {
            out.push(body.clone());
            *body_idx += 1;
        }
    }
    out
}

fn is_empty_part(line: &[u8], start: usize, i: usize) -> bool {
    line.get(start..i).unwrap_or(b"").trim_ascii().is_empty()
}

#[cfg(test)]
mod tests;
