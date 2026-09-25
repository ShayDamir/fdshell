//! Strict sequence term parsing (port of bash's `expand_seqterm`).

use super::{SeqKind, SeqSpec, parse_int, parse_int_prefix};

/// Parse a sequence term strictly; `None` when invalid. `amble` is the group
/// content without the braces.
pub fn expand_seqterm(amble: &[u8]) -> Option<SeqSpec> {
    let lhs_l = amble.windows(2).position(|w| w == b"..".as_slice())?;
    let lhs = amble.get(..lhs_l)?;
    let rhs = amble.get(lhs_l + 2..)?;
    // An empty side is rejected by the arms below (`parse_int` fails,
    // `first()` is `None`), so no separate empty check.
    let (start, mut kind) = match parse_int(lhs) {
        Some(v) => (v, SeqKind::Int),
        None => match lhs {
            [c] if c.is_ascii_alphabetic() => (*c as i64, SeqKind::Char),
            _ => return None,
        },
    };
    let (end, mut ep, rhs_kind) = match rhs.first().copied() {
        // A sign without digits (or a non-digit first byte) fails
        // `parse_int_prefix`, which returns `None` like the `_` arm.
        Some(r0) if r0.is_ascii_digit() || matches!(r0, b'+' | b'-') => {
            let (v, e) = parse_int_prefix(rhs)?;
            match rhs.get(e).copied() {
                None | Some(b'.') => (v, e, SeqKind::Int),
                Some(_) => return None,
            }
        }
        // Junk after the single char (including a lone `.`) is rejected by
        // the trailing-`ep == rhs.len()` check below.
        Some(r0) if r0.is_ascii_alphabetic() => (r0 as i64, 1, SeqKind::Char),
        _ => return None,
    };
    if kind != rhs_kind {
        return None;
    }
    let mut incr = 1;
    let oep = ep;
    if matches!(rhs.get(ep), Some(&b'.'))
        && matches!(rhs.get(ep + 1), Some(&b'.'))
        && rhs.get(ep + 2).is_some()
    {
        let rest = rhs.get(ep + 2..)?;
        let (v, n) = parse_int_prefix(rest)?;
        ep += 2 + n;
        incr = v;
    }
    if ep != rhs.len() {
        return None;
    }
    // Zero-padding (bash ST_ZINT): a side is padded when it starts with `0`
    // or `-0` and has at least one more byte; the width is the max of both
    // sides' lengths.
    let mut width = 0usize;
    if kind == SeqKind::Int {
        let rhs_l = amble.len() - (ep - oep) - lhs_l - 2;
        let lhs_padded = (lhs_l > 1 && lhs.first() == Some(&b'0'))
            || (lhs_l > 2 && lhs.first() == Some(&b'-') && lhs.get(1) == Some(&b'0'));
        let rhs_padded = (rhs_l > 1 && rhs.first() == Some(&b'0'))
            || (rhs_l > 2 && rhs.first() == Some(&b'-') && rhs.get(1) == Some(&b'0'));
        if lhs_padded || rhs_padded {
            width = lhs_l.max(rhs_l);
            kind = SeqKind::ZInt;
        }
    }
    Some(SeqSpec {
        start,
        end,
        incr,
        kind,
        width,
    })
}
