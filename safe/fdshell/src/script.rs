use crate::error::parse::ParseErrorInfo;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

fn boundary(word: &[u8], len: usize, extra: &[u8]) -> bool {
    word.get(len)
        .is_none_or(|&b| b.is_ascii_whitespace() || b == b';' || extra.contains(&b))
}

fn keyword_delta(word: &[u8]) -> Option<i32> {
    if word.starts_with(b"if") && boundary(word, 2, b"") {
        return Some(1);
    }
    if word.starts_with(b"fi") && boundary(word, 2, b"&|") {
        return Some(-1);
    }
    if word.starts_with(b"for") && boundary(word, 3, b"") {
        return Some(1);
    }
    if (word.starts_with(b"while") || word.starts_with(b"until")) && boundary(word, 5, b"") {
        return Some(1);
    }
    if word.starts_with(b"done") && boundary(word, 4, b"&|") {
        return Some(-1);
    }
    None
}

pub(crate) fn run_script(line: &[u8], cell: &ForkCell<ShellState>) -> Result<i32, ParseErrorInfo> {
    let mut start = 0;
    let mut in_quote = false;
    let mut i = 0;
    while i <= line.len() {
        if line.get(i) == Some(&b'"') {
            in_quote = !in_quote;
        } else if i == line.len()
            || (!in_quote && matches!(line.get(i), Some(&b';') | Some(&b'\n')))
        {
            let part = line.get(start..i).unwrap_or(b"").trim_ascii();
            if !part.is_empty() && keyword_delta(part) == Some(1) {
                let block_start = start;
                let mut depth = 1u32;
                // Find keyword position in original line to skip it when scanning
                let original = line.get(block_start..i).unwrap_or(b"");
                let leading_ws = original
                    .iter()
                    .take_while(|&&b| b.is_ascii_whitespace())
                    .count();
                let kw = if part.starts_with(b"if") {
                    2
                } else if part.starts_with(b"for") {
                    3
                } else {
                    5 // while or until
                };
                i = block_start + leading_ws + kw;
                start = i;
                while i <= line.len() && depth > 0 {
                    if line.get(i) == Some(&b'"') {
                        in_quote = !in_quote;
                    } else if i == line.len()
                        || (!in_quote && matches!(line.get(i), Some(&b';') | Some(&b'\n')))
                    {
                        let raw = line.get(start..i).unwrap_or(b"").trim_ascii();
                        for sub in raw.split(|&b| b == b' ') {
                            if !sub.is_empty() {
                                match keyword_delta(sub) {
                                    Some(1) => depth += 1,
                                    Some(-1) => depth -= 1,
                                    _ => {}
                                }
                            }
                        }
                        start = i + 1;
                    }
                    i += 1;
                }
                if depth > 0 {
                    return Err(ParseErrorInfo::new(block_start, "missing 'fi'"));
                }
                let end = line.len().min(start);
                let full = line.get(block_start..end).unwrap_or(b"").trim_ascii();
                crate::cond::run_cond_list(full, cell)?;
                continue;
            }
            if !part.is_empty() {
                crate::cond::run_cond_list(part, cell)?;
            }
            start = i + 1;
        }
        i += 1;
    }
    let state = cell.borrow()?;
    Ok(state.last_status.exit_code())
}
