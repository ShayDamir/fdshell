use error_stack::Report;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

mod op;

pub(in crate::substitute) use op::ParamOp;

pub(super) mod colon;
pub(super) mod pattern;

/// Splits `${name<op>word}` content at the first operator, scanned
/// left-to-right so the precedence is positional: `${v#a:}` takes the pattern
/// `a:` (bash), `${var:x:-w}` takes the colon word `w` (a lone `:` is no
/// operator), and `${v:-x#y}` stops at the colon so `#` is part of its word.
pub(super) fn split_operator(content: &ShortCStr) -> Option<(ShortCStr, ParamOp, ShortCStr)> {
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

/// Applies the operator, appending the expansion to `out`. `mask` is the word's
/// quote mask and `content_start` the word index of the first braced-content
/// byte, so the pattern arm can slice the mask under its pattern bytes.
pub(super) fn apply_param_op(
    name: &ShortCStr,
    op: ParamOp,
    word: &ShortCStr,
    mask: &[bool],
    content_start: usize,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    match op {
        ParamOp::Default | ParamOp::Alternate | ParamOp::Error => {
            colon::apply(name, op, word, cell, out)
        }
        ParamOp::Assign => colon::assign(name, word, cell, out),
        ParamOp::ShortestPrefix
        | ParamOp::LongestPrefix
        | ParamOp::ShortestSuffix
        | ParamOp::LongestSuffix => pattern::apply(name, op, word, mask, content_start, cell, out),
    }
}
