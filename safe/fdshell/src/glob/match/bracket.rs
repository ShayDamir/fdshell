//! Bracket expressions `[...]`: span validity and membership.
//!
//! After `[`, an unquoted `!` negates; a `]` as the first byte is a literal
//! member; the next unquoted `]` closes. Quoted bytes and unquoted `\X` pairs
//! are literal members. A range is an unquoted, unescaped `a-z`; an invalid
//! range (start after end) matches nothing, and scanning stops when the byte
//! right after the range end is `]`, otherwise that byte is the next member
//! (bash `BRACKMATCH` semantics).

mod span;

pub(in crate::glob) use span::{quoted, span};

/// Whether `b` is a member of the (valid) bracket expression at `at_open`.
pub(super) fn contains(bytes: &[u8], mask: &[bool], at_open: usize, b: u8) -> bool {
    let mut i = at_open + 1;
    let mut negated = false;
    if bytes.get(i) == Some(&b'!') && !quoted(mask, i) {
        negated = true;
        i += 1;
    }
    let Some(mut c) = bytes.get(i).copied() else {
        return negated;
    };
    i += 1;
    loop {
        let mut start = c;
        let mut end = c;
        let mut plain = !quoted(mask, i - 1);
        if plain && c == b'\\' {
            let Some(n) = bytes.get(i).copied() else {
                break;
            };
            start = n;
            end = n;
            plain = false;
            i += 1;
        }
        let Some(nxt) = bytes.get(i).copied() else {
            break;
        };
        let dash_i = i;
        i += 1;
        if let Some(e) = bytes.get(i).copied()
            && e != b']'
            && !quoted(mask, i)
            && e != b'\\'
            && nxt == b'-'
            && !quoted(mask, dash_i)
            && plain
        {
            end = e;
            i += 1;
            let Some(after) = bytes.get(i).copied() else {
                break;
            };
            i += 1;
            if start > end {
                if after == b']' {
                    return negated;
                }
                c = after;
                continue;
            }
            if start <= b && b <= end {
                return !negated;
            }
            if after == b']' {
                break;
            }
            c = after;
            continue;
        }
        if start <= b && b <= end {
            return !negated;
        }
        if nxt == b']' && !quoted(mask, dash_i) {
            break;
        }
        c = nxt;
    }
    negated
}
