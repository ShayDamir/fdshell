mod boundary;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::scan::ScanState;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

pub(crate) fn run_cond_list(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let mut state = ScanState::new();
    let mut start = 0;
    loop {
        let (i, is_or) = boundary::next_boundary(line, start, &mut state);
        if i == line.len() {
            if let Some(control) = run_part(text, line, start, i, cell)? {
                return Ok(Some(control));
            }
            break;
        }
        if !is_empty_part(line, start, i) {
            if let Some(control) = run_part(text, line, start, i, cell)? {
                return Ok(Some(control));
            }
            let st = cell.borrow().change_context(CmdError::Never)?;
            let ok = st.last_status.exit_code() == 0;
            if is_or && ok {
                return Ok(None);
            }
            if !is_or && !ok {
                // `&&` after a failure: skip the rest of the list up to `||`.
                start = skip_to_or(line, i + 2, &mut state);
                continue;
            }
        }
        start = i + 2;
    }
    Ok(None)
}

/// After a failing `&&` part, the next `||` (or end of line): everything up
/// to it is skipped without running.
fn skip_to_or(line: &[u8], from: usize, state: &mut ScanState) -> usize {
    let mut pos = from;
    loop {
        let (p, is_or) = boundary::next_boundary(line, pos, state);
        if is_or || p == line.len() {
            return p;
        }
        pos = p + 2;
    }
}

/// Run the conditional part spanning `line[start..i]` as a subsliced statement.
fn run_part(
    text: &ScriptText,
    line: &[u8],
    start: usize,
    i: usize,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let raw = line.get(start..i).unwrap_or(b"");
    if raw.trim_ascii().is_empty() {
        return Ok(None);
    }
    let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
    // Trailing bytes are kept: an empty-delimiter heredoc ends in the blank
    // line's newline, which the parser needs to find the delimiter line.
    let part_text = text
        .subslice(start + lead, i - start - lead)
        .ok_or(CmdError::Never)?;
    crate::run::run_one(&part_text, cell)
}

fn is_empty_part(line: &[u8], start: usize, i: usize) -> bool {
    line.get(start..i).unwrap_or(b"").trim_ascii().is_empty()
}
