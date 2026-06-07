use crate::state::ShellState;
use sys::errno::EINVAL;

fn is_if_or_fi(word: &[u8]) -> Option<bool> {
    if word.starts_with(b"if")
        && word
            .get(2)
            .is_none_or(|&b| b.is_ascii_whitespace() || b == b';')
    {
        return Some(true);
    }
    if word.starts_with(b"fi")
        && word
            .get(2)
            .is_none_or(|&b| b.is_ascii_whitespace() || matches!(b, b';' | b'&' | b'|'))
    {
        return Some(false);
    }
    None
}

pub(crate) fn run_script(line: &[u8], state: &mut ShellState) -> Result<i32, i32> {
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
            if !part.is_empty() {
                if is_if_or_fi(part) == Some(true) {
                    let if_start = start;
                    let mut depth = 1u32;
                    start = i + 1;
                    i += 1;
                    while i <= line.len() && depth > 0 {
                        if line.get(i) == Some(&b'"') {
                            in_quote = !in_quote;
                        } else if i == line.len()
                            || (!in_quote && matches!(line.get(i), Some(&b';') | Some(&b'\n')))
                        {
                            let raw = line.get(start..i).unwrap_or(b"").trim_ascii();
                            for sub in raw.split(|&b| b == b' ') {
                                if !sub.is_empty() {
                                    match is_if_or_fi(sub) {
                                        Some(true) => depth += 1,
                                        Some(false) => depth -= 1,
                                        None => {}
                                    }
                                }
                            }
                            start = i + 1;
                        }
                        i += 1;
                    }
                    if depth > 0 {
                        return Err(EINVAL);
                    }
                    let end = line.len().min(start);
                    let full = line.get(if_start..end).unwrap_or(b"").trim_ascii();
                    crate::cond::run_cond_list(full, state)?;
                    continue;
                }
                crate::cond::run_cond_list(part, state)?;
            }
            start = i + 1;
        }
        i += 1;
    }
    Ok(state.last_status.exit_code())
}
