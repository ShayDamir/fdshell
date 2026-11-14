//! The positional scan of a braced body (`param_op.rs`), split out for the line
//! budget: the operator precedence is positional, so the scan walks
//! left-to-right and the first operator wins.

// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -- -E 'test(/^substitute::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes)'

use sys::ShortCStr;

use super::op::ParamOp;

/// Splits `${name<op>word}` content at the first operator, scanned
/// left-to-right so the precedence is positional: `${v#a:}` takes the pattern
/// `a:` (bash), `${var:x:-w}` takes the colon word `w` (a lone `:` is no
/// operator), and `${v:-x#y}` stops at the colon so `#` is part of its word.
pub(in crate::substitute) fn split_operator(
    content: &ShortCStr,
) -> Option<(ShortCStr, ParamOp, ShortCStr)> {
    let bytes = content.as_bytes().ok()?;
    let mut i = 0usize;
    loop {
        let b0 = bytes.get(i).copied()?;
        let b1 = bytes.get(i + 1).copied();
        if i > 0
            && b0 == b':'
            && let Some(op) = colon_op(b1)
        {
            return Some((content.get(0..i)?, op, content.get(i + 2..)?));
        }
        if matches!(b0, b'#' | b'%') {
            let longest = b1 == Some(b0);
            let skip = 1 + usize::from(longest);
            return Some((
                content.get(0..i)?,
                pattern_op(b0, longest),
                content.get(i + skip..)?,
            ));
        }
        i += 1;
    }
}

/// The byte after a `:` is a colon operator only when it is one of the four;
/// a lone `:` (or `:x`) is no operator, so the scan continues.
fn colon_op(op: Option<u8>) -> Option<ParamOp> {
    match op {
        Some(b'-') => Some(ParamOp::Default),
        Some(b'=') => Some(ParamOp::Assign),
        Some(b'+') => Some(ParamOp::Alternate),
        Some(b'?') => Some(ParamOp::Error),
        _ => None,
    }
}

/// `#`/`%` with the doubled byte as the long form (`##`, `%%`).
fn pattern_op(op: u8, longest: bool) -> ParamOp {
    match (op, longest) {
        (b'#', false) => ParamOp::ShortestPrefix,
        (b'#', true) => ParamOp::LongestPrefix,
        (b'%', false) => ParamOp::ShortestSuffix,
        _ => ParamOp::LongestSuffix,
    }
}
