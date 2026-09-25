//! The brace-group scanner (port of bash's `brace_gobbler`). Walks raw token
//! bytes with its own quote state; returns where a group closes and what
//! kind of group it was.

use crate::paren_scan::scan_paren_body;

/// The separator class of a found `}` group.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub(super) enum GroupType {
    /// A top-level unquoted comma (a comma outranks `..`).
    Comma,
    /// A top-level unquoted `..` with no comma.
    Seq,
}

/// Scan `text` from `from` for the first `satisfy` byte that ends a brace
/// group; return `(index, found, etype)`. `etype` is set only for `}`.
pub(super) fn gobble(text: &[u8], from: usize, satisfy: u8) -> (usize, bool, Option<GroupType>) {
    let mut i = from;
    let mut level = 0u32;
    let mut btype: Option<GroupType> = None;
    let mut commas = usize::from(satisfy != b'}');
    let mut in_quote = false;
    let mut in_backtick = false;
    while let Some(&c) = text.get(i) {
        if in_quote || in_backtick {
            let closer = if in_quote { b'"' } else { b'`' };
            if c == b'\\' {
                i += 2;
            } else if c == closer {
                if in_quote {
                    in_quote = false;
                } else {
                    in_backtick = false;
                }
                i += 1;
            } else {
                i += 1;
            }
            continue;
        }
        match c {
            b'"' => in_quote = true,
            b'`' => in_backtick = true,
            b'$' if text.get(i + 1) == Some(&b'{') => {
                i += 2;
                level += 1;
                continue;
            }
            b'$' if text.get(i + 1) == Some(&b'(') => {
                i = skip_dollar_paren(text, i);
                continue;
            }
            _ => {}
        }
        if c == satisfy && level == 0 && commas > 0 {
            if c == b'{' && ignore_open_brace(text, i) {
                i += 1;
                continue;
            }
            let etype = if satisfy == b'}' { btype } else { None };
            return (i, true, etype);
        }
        if c == b'{' {
            level += 1;
        } else if c == b'}' && level > 0 {
            level -= 1;
        } else if satisfy == b'}' && c == b',' && level == 0 {
            btype = Some(GroupType::Comma);
            commas += 1;
        } else if satisfy == b'}' && seq_sep(text, i) && level == 0 && btype.is_none() {
            btype = Some(GroupType::Seq);
            commas += 1;
        }
        i += 1;
    }
    (text.len(), false, None)
}

/// Whether `..` starts at `i` and can begin a sequence (next byte is not `}`).
fn seq_sep(text: &[u8], i: usize) -> bool {
    match text.get(i..i + 2) {
        Some(s) if s == (b"..").as_slice() => text.get(i + 2) != Some(&b'}'),
        _ => false,
    }
}

/// The bash rule: a top-level `{` is ignored when preceded by word start or
/// whitespace and followed by whitespace or `}` (so `{a, b}` is not a group).
fn ignore_open_brace(text: &[u8], i: usize) -> bool {
    let prev_blank = i == 0 || text.get(i - 1).is_some_and(|&b| is_blank(b));
    let next = text.get(i + 1);
    prev_blank && (next.is_none_or(|&b| is_blank(b)) || next == Some(&b'}'))
}

fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n')
}

/// Skip a `$(…)` span starting at `i`; return the index just past the
/// matching `)`. Reuses the tokenizer's scanner for exact parity. The caller
/// just checked the `(` at `i + 1`, so `rest` exists (possibly empty); an
/// unterminated span scans to the end of the word and swallows the rest.
fn skip_dollar_paren(text: &[u8], i: usize) -> usize {
    let rest = text.get(i + 2..).unwrap_or_default();
    match scan_paren_body(&mut rest.iter().copied().peekable(), 1) {
        // `$(` + body + `)`: the span is `3 + body.len()` bytes long.
        Some(body) => i + 3 + body.len(),
        None => text.len(),
    }
}
