use alloc::vec::Vec;

use crate::bytes::QUOTE;

/// Scan a `` ` … ` `` region in `line`; `start` is the index of the opening
/// backtick. Returns the body text (the backtick excluded, quotes dropped), the
/// number of raw bytes consumed from `start` (the closing backtick included),
/// and whether the region closed. There is no nesting: the region ends at the
/// first unescaped, unquoted backtick.
///
/// An escape pair `\X` shields the next byte and is kept in the body, so
/// `` \` `` never closes the region. A `"` toggles the quote state and is
/// dropped from the body (a quote is a marker, not content). A `"` left
/// unpaired at the end of the line belongs to the *enclosing* region: the scan
/// stops before it, so the enclosing quote can close and the unterminated
/// region stays literal text.
pub(crate) fn scan_backtick(line: &[u8], start: usize) -> (Vec<u8>, usize, bool) {
    let mut body = Vec::new();
    let mut in_quotes = false;
    let mut open_quote: Option<(usize, usize)> = None;
    let mut i = start.saturating_add(1); // just past the opening backtick
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
        if c == b'`' && !in_quotes {
            return (body, i - start, true);
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

/// Index just past the backtick that closes the region at `start` (the index
/// of the opening backtick), or `None` when the region does not close — the
/// caller then falls back to byte-at-a-time scanning.
pub(crate) fn scan_backtick_at(line: &[u8], start: usize) -> Option<usize> {
    let (_, consumed, closed) = scan_backtick(line, start);
    closed.then_some(start + consumed)
}
