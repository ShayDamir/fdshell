/// Skip from `i` past a `#` comment to the next `\n` (or end of input).
/// Returns the index to resume scanning from.
pub(crate) fn skip_comment(line: &[u8], mut i: usize) -> usize {
    while i <= line.len() {
        if line.get(i) == Some(&b'\n') {
            i += 1;
            break;
        }
        i += 1;
    }
    i
}

/// Scan forward from `i` looking for the matching closing keyword (depth == 0).
/// Updates `in_quote` and `start` as side effects.
/// Returns `(end_position, block_was_closed)`.
pub(crate) fn scan_block(
    line: &[u8],
    mut i: usize,
    in_quote: &mut bool,
    start: &mut usize,
    mut depth: u32,
) -> (usize, bool) {
    let mut dollar_paren_depth = 0u32;
    let mut in_backtick = false;
    while i <= line.len() && depth > 0 {
        let is_comment = !*in_quote && !in_backtick && line.get(i) == Some(&b'#');
        let is_sep = i == line.len()
            || (!*in_quote
                && !in_backtick
                && dollar_paren_depth == 0
                && matches!(line.get(i), Some(&b';') | Some(&b'\n')));

        if is_comment || is_sep {
            let raw = line.get(*start..i).unwrap_or(b"").trim_ascii();
            for sub in raw.split(|&b| b == b' ') {
                if !sub.is_empty() {
                    match crate::keywords::keyword_delta(sub) {
                        Some(1) => depth += 1,
                        Some(-1) => depth -= 1,
                        _ => {}
                    }
                }
            }
            if is_comment {
                i = skip_comment(line, i);
                *start = i;
                continue;
            } else {
                *start = i + 1;
            }
        } else if line.get(i) == Some(&b'"') {
            *in_quote = !*in_quote;
        } else if !*in_quote && !in_backtick && line.get(i) == Some(&b'$') {
            if line.get(i + 1) == Some(&b'(') {
                dollar_paren_depth = dollar_paren_depth.saturating_add(1);
                i += 1;
            }
        } else if !*in_quote && !in_backtick && line.get(i) == Some(&b'(') {
            if dollar_paren_depth > 0 {
                dollar_paren_depth = dollar_paren_depth.saturating_add(1);
            }
        } else if !*in_quote && !in_backtick && line.get(i) == Some(&b')') {
            dollar_paren_depth = dollar_paren_depth.saturating_sub(1);
        } else if !*in_quote && !in_backtick && line.get(i) == Some(&b'`') {
            in_backtick = true;
        } else if in_backtick && line.get(i) == Some(&b'`') {
            in_backtick = false;
        }
        i += 1;
    }
    (i, depth == 0)
}

#[cfg(test)]
mod tests;
