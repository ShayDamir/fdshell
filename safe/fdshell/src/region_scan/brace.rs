use alloc::vec::Vec;

use crate::bytes::QUOTE;

/// Scan a `${ … }` region in `line`; `start` is the index of the `$`. Returns
/// the body text (the `${` prefix excluded, quotes dropped), the number of raw
/// bytes consumed from `start` (the closing `}` included), and whether the
/// region closed.
///
/// An escape pair `\X` shields the next byte and is kept in the body, so an
/// escaped `}`, `)` or quote never counts. Brace and paren depths are counted
/// together, so a `}` inside a nested `${ … }` or inside a `$( … )` region is
/// data and only the `}` that closes both depths ends the region. A `"`
/// toggles the quote state and is dropped from the body: a quote is a marker,
/// not content, because the substitution pass takes the braced content
/// verbatim. A `"` left unpaired at the end of the line belongs to the
/// *enclosing* region: the scan stops before it, so the enclosing quote can
/// close and the unterminated `${ … }` stays literal text.
pub(crate) fn scan_brace(line: &[u8], start: usize) -> (Vec<u8>, usize, bool) {
    let mut body = Vec::new();
    let mut depth: u32 = 0;
    let mut in_quotes = false;
    let mut open_quote: Option<(usize, usize)> = None;
    let mut i = start.saturating_add(2); // just past `${`
    while let Some(&c) = line.get(i) {
        i += 1;
        if c == b'\\' {
            let Some(&escaped) = line.get(i) else { break };
            i += 1;
            body.push(b'\\');
            body.push(escaped);
            continue;
        }
        if c == QUOTE {
            in_quotes = !in_quotes;
            open_quote = if in_quotes {
                Some((i - 1 - start, body.len()))
            } else {
                None
            };
            continue;
        }
        if in_quotes {
            body.push(c);
            continue;
        }
        if c == b'{' || c == b'(' {
            depth = depth.saturating_add(1);
            body.push(c);
            continue;
        }
        if c == b'}' && depth == 0 {
            return (body, i - start, true);
        }
        if c == b'}' || c == b')' {
            depth = depth.saturating_sub(1);
        }
        body.push(c);
    }
    match open_quote {
        Some((at, len)) => {
            body.truncate(len);
            (body, at, false)
        }
        None => (body, line.len().saturating_sub(start), false),
    }
}

/// Index just past the `}` that closes the `${ … }` region at `start` (the
/// index of the `$`), or `None` when the region does not close — the caller
/// then falls back to byte-at-a-time scanning.
pub(crate) fn scan_brace_at(line: &[u8], start: usize) -> Option<usize> {
    let (_, consumed, closed) = scan_brace(line, start);
    closed.then_some(start + consumed)
}
