use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use sys::ScriptText;
use sys::fork_cell::ForkCell;

pub(crate) fn run_cond_list(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let mut start = 0;
    let mut in_quote = false;
    let mut i = 0;
    while i <= line.len() {
        if line.get(i) == Some(&b'"') {
            in_quote = !in_quote;
        } else if i == line.len() {
            if let Some(control) = run_part(text, line, start, i, cell)? {
                return Ok(Some(control));
            }
            break;
        } else if !in_quote {
            let tail = line.get(i..).unwrap_or(b"");
            if tail.starts_with(b"&&") || tail.starts_with(b"||") {
                if !is_empty_part(line, start, i) {
                    if let Some(control) = run_part(text, line, start, i, cell)? {
                        return Ok(Some(control));
                    }
                    let state = cell.borrow().change_context(CmdError::Never)?;
                    if tail.starts_with(b"&&") && state.last_status.exit_code() != 0 {
                        let mut j = i + 2;
                        let mut q = false;
                        while j <= line.len() {
                            if line.get(j) == Some(&b'"') {
                                q = !q;
                            } else if (!q
                                && line.get(j..) != Some(b"")
                                && line.get(j..).unwrap_or(b"").starts_with(b"||"))
                                || j == line.len()
                            {
                                start = j;
                                i = j;
                                break;
                            }
                            j += 1;
                        }
                        continue;
                    }
                    if tail.starts_with(b"||") && state.last_status.exit_code() == 0 {
                        return Ok(None);
                    }
                }
                start = i + 2;
                i = start;
                continue;
            }
        }
        i += 1;
    }
    Ok(None)
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
    let part = raw.trim_ascii();
    if part.is_empty() {
        return Ok(None);
    }
    let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
    let part_text = text
        .subslice(start + lead, part.len())
        .ok_or(CmdError::Never)?;
    crate::run::run_one(&part_text, cell)
}

fn is_empty_part(line: &[u8], start: usize, i: usize) -> bool {
    line.get(start..i).unwrap_or(b"").trim_ascii().is_empty()
}
