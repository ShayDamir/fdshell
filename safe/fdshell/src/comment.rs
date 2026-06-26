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
    while i <= line.len() && depth > 0 {
        if !*in_quote && line.get(i) == Some(&b'#') {
            i = skip_comment(line, i);
            *start = i;
            continue;
        }
        if line.get(i) == Some(&b'"') {
            *in_quote = !*in_quote;
        } else if i == line.len()
            || (!*in_quote && matches!(line.get(i), Some(&b';') | Some(&b'\n')))
        {
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
            *start = i + 1;
        }
        i += 1;
    }
    (i, depth == 0)
}
