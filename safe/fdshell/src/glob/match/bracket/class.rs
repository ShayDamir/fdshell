//! POSIX character classes `[[:name:]]` inside bracket expressions.
//!
//! C-locale (ASCII) semantics. An unquoted `[:name:]` is an atomic member of
//! the enclosing `[...]`; an unrecognized `name` (or an unclosed `[:`) is not
//! a class — its bytes are literal members (bash behavior).

const UPPER: core::ops::RangeInclusive<u8> = b'A'..=b'Z';
const LOWER: core::ops::RangeInclusive<u8> = b'a'..=b'z';
const DIGIT: core::ops::RangeInclusive<u8> = b'0'..=b'9';

/// True when an unquoted `[:` begins a candidate class at `i` (the `[`).
pub(in crate::glob) fn is_class_start(bytes: &[u8], mask: &[bool], i: usize) -> bool {
    !super::quoted(mask, i) && bytes.get(i) == Some(&b'[') && bytes.get(i + 1) == Some(&b':')
}

/// The index just past the closing `:]` of the class opened by `[:` at `i`,
/// or `None` when no `:]` follows (the bytes are literal members).
pub(in crate::glob) fn class_end(bytes: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 2;
    while let Some(&b) = bytes.get(j) {
        if b == b':' && bytes.get(j + 1) == Some(&b']') {
            return Some(j + 2);
        }
        j += 1;
    }
    None
}

/// The `(close, member)` pair for the `[:class:]` candidate opened by the
/// unquoted `[` at `i`, where `close` is the index just past the class.
/// `None` when the candidate is not a recognized class (an unclosed `[:`,
/// an unrecognized name) — the caller treats its bytes as literal members.
pub(in crate::glob) fn class_member(
    bytes: &[u8],
    mask: &[bool],
    i: usize,
    b: u8,
) -> Option<(usize, bool)> {
    let open = i - 1;
    if !is_class_start(bytes, mask, open) {
        return None;
    }
    let close = class_end(bytes, open)?;
    let name = bytes.get(open + 2..close - 2)?;
    class_contains(name, b).map(|member| (close, member))
}

/// Whether `b` is a member of the named class; `None` for an unrecognized
/// name (the caller treats the class bytes as literal members).
pub(in crate::glob) fn class_contains(name: &[u8], b: u8) -> Option<bool> {
    let alpha = UPPER.contains(&b) || LOWER.contains(&b);
    let digit = DIGIT.contains(&b);
    match name {
        b"alpha" | b"alphabetic" => Some(alpha),
        b"digit" | b"numeric" => Some(digit),
        b"alnum" => Some(alpha || digit),
        b"upper" | b"uppercase" => Some(UPPER.contains(&b)),
        b"lower" | b"lowercase" => Some(LOWER.contains(&b)),
        b"xdigit" | b"hexdigit" => {
            Some(digit || (b'A'..=b'F').contains(&b) || (b'a'..=b'f').contains(&b))
        }
        b"space" => Some(matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0C | 0x0B)),
        b"blank" => Some(b == b' ' || b == b'\t'),
        b"cntrl" | b"control" => Some(b < 0x20 || b == 0x7F),
        b"graph" => Some((0x21..=0x7E).contains(&b)),
        b"print" | b"printable" => Some((0x20..=0x7E).contains(&b)),
        b"del" => Some(b == 0x7F),
        b"punct" | b"punctuation" => Some((0x21..=0x7E).contains(&b) && !alpha && !digit),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
