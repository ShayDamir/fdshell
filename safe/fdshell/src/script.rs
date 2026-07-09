use crate::comment::{scan_block, skip_comment};
use crate::keywords::keyword_delta;
use crate::loop_control::LoopControl;
use error_stack::{Report, ensure};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_script(
    line: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let mut start = 0;
    let mut in_quote = false;
    let mut i = 0;
    while i <= line.len() {
        if !in_quote && line.get(i) == Some(&b'#') {
            i = skip_comment(line, i);
            start = i;
            continue;
        }
        if line.get(i) == Some(&b'"') {
            in_quote = !in_quote;
        } else if i == line.len()
            || (!in_quote && matches!(line.get(i), Some(&b';') | Some(&b'\n')))
        {
            let part = line.get(start..i).unwrap_or(b"").trim_ascii();
            if !part.is_empty() && keyword_delta(part) == Some(1) {
                let block_start = start;
                let original = line.get(block_start..i).unwrap_or(b"");
                let leading_ws = original
                    .iter()
                    .take_while(|&&b| b.is_ascii_whitespace())
                    .count();
                let kw_len = match part {
                    p if p.starts_with(b"case") => 4,
                    p if p.starts_with(b"if") => 2,
                    p if p.starts_with(b"for") => 3,
                    _ => 5,
                };
                i = block_start + leading_ws + kw_len;
                start = i;
                let (end_pos, closed) = scan_block(line, i, &mut in_quote, &mut start, 1);
                ensure!(closed, CmdError::Parse);
                i = end_pos;
                let end = line.len().min(start.saturating_sub(1));
                let full = line.get(block_start..end).unwrap_or(b"").trim_ascii();
                if let Some(control) = crate::cond::run_cond_list(full, cell)? {
                    return Ok(Some(control));
                }
                continue;
            }
            if !part.is_empty()
                && let Some(control) = crate::cond::run_cond_list(part, cell)?
            {
                return Ok(Some(control));
            }
            start = i + 1;
        }
        i += 1;
    }
    Ok(None)
}
