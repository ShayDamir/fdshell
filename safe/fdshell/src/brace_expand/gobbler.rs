//! The brace-group scanner (port of bash's `brace_gobbler`). Walks raw token
//! bytes with its own quote state; returns where a group closes and what
//! kind of group it was.

mod rules;

use crate::bytes::QUOTE;

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
            let closer = if in_quote { QUOTE } else { b'`' };
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
            QUOTE => in_quote = true,
            b'`' => in_backtick = true,
            // An unquoted escape pair is skipped whole: an escaped `{`, `}` or
            // `,` never opens a group or separates one.
            b'\\' => {
                i += 2;
                continue;
            }
            b'$' if text.get(i + 1) == Some(&b'{') => {
                i += 2;
                level += 1;
                continue;
            }
            b'$' if text.get(i + 1) == Some(&b'(') => {
                i = rules::skip_dollar_paren(text, i);
                continue;
            }
            _ => {}
        }
        if c == satisfy && level == 0 && commas > 0 {
            if c == b'{' && rules::ignore_open_brace(text, i) {
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
        } else if satisfy == b'}' && rules::seq_sep(text, i) && level == 0 && btype.is_none() {
            btype = Some(GroupType::Seq);
            commas += 1;
        }
        i += 1;
    }
    (text.len(), false, None)
}
