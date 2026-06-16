use crate::error::parse::ParseErrorInfo;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_cond_list(
    line: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<(), ParseErrorInfo> {
    let mut start = 0;
    let mut in_quote = false;
    let mut i = 0;
    while i <= line.len() {
        if line.get(i) == Some(&b'"') {
            in_quote = !in_quote;
        } else if i == line.len() {
            let part = line.get(start..i).unwrap_or(b"").trim_ascii();
            if !part.is_empty() {
                crate::run::run_one(part, cell)?;
            }
            break;
        } else if !in_quote {
            let tail = line.get(i..).unwrap_or(b"");
            if tail.starts_with(b"&&") || tail.starts_with(b"||") {
                let part = line.get(start..i).unwrap_or(b"").trim_ascii();
                if !part.is_empty() {
                    crate::run::run_one(part, cell)?;
                    let state = cell.borrow()?;
                    if (tail.starts_with(b"&&") && state.last_status.exit_code() != 0)
                        || (tail.starts_with(b"||") && state.last_status.exit_code() == 0)
                    {
                        return Ok(());
                    }
                }
                start = i + 2;
                i += 1;
            }
        }
        i += 1;
    }
    Ok(())
}
