//! Per-word brace expansion (port of bash's `brace_expand`).

pub(super) mod concat;

use super::gobbler::{GroupType, gobble};
use super::seq::{expand_seqterm, mkseq, valid_seqterm};
use crate::error::parse::ParseError;
use alloc::vec;
use alloc::vec::Vec;
use concat::cross;
use concat::expand_amble;
use error_stack::Report;

/// The first valid brace group in `text` as `(open, close, type)`. The
/// gobbler reports an `etype` only when the closing `}` was found, so one
/// check covers both.
fn find_group(text: &[u8]) -> Option<(usize, usize, GroupType)> {
    let mut i = 0usize;
    loop {
        let (gi, found_open, _) = gobble(text, i, b'{');
        if !found_open {
            return None;
        }
        let (close, t) = match gobble(text, gi + 1, b'}') {
            (close, true, Some(t)) => (close, t),
            _ => {
                i = gi + 1;
                continue;
            }
        };
        if t == GroupType::Seq {
            let amble = text.get(gi + 1..close)?;
            if !valid_seqterm(amble) {
                i = gi + 1;
                continue;
            }
        }
        return Some((gi, close, t));
    }
}

/// Brace-expand one raw token span; `Ok(None)` when the word is unchanged.
pub(super) fn expand_word(text: &[u8]) -> Result<Option<Vec<Vec<u8>>>, Report<ParseError>> {
    let Some((gi, close, t)) = find_group(text) else {
        return Ok(None);
    };
    // The gobbler guarantees `0 <= gi < close < text.len()`, so every slice
    // below is in range; `Never` marks a logic error if that ever breaks.
    let preamble = text.get(..gi).ok_or(ParseError::Never)?;
    let amble = text.get(gi + 1..close).ok_or(ParseError::Never)?;
    let group = text.get(gi..=close).ok_or(ParseError::Never)?;
    let postamble = text.get(close + 1..).ok_or(ParseError::Never)?;

    let amble_words = match t {
        GroupType::Comma => expand_amble(amble)?,
        GroupType::Seq => match expand_seqterm(amble) {
            Some(spec) => match mkseq(&spec)? {
                Some(words) => words,
                None => return seq_literal(preamble, group, postamble),
            },
            None => return seq_literal(preamble, group, postamble),
        },
    };

    let mut result = cross(vec![preamble.to_vec()], amble_words)?;
    if !postamble.is_empty() {
        let post_words = match expand_word(postamble)? {
            Some(words) => words,
            None => vec![postamble.to_vec()],
        };
        result = cross(result, post_words)?;
    }
    Ok(Some(result))
}

/// A sequence group that failed to expand stays literal (braces included);
/// a non-empty postamble is still expanded. No postamble: the word is
/// unchanged (`Ok(None)`).
fn seq_literal(
    preamble: &[u8],
    group: &[u8],
    postamble: &[u8],
) -> Result<Option<Vec<Vec<u8>>>, Report<ParseError>> {
    if postamble.is_empty() {
        return Ok(None);
    }
    let base = cross(vec![preamble.to_vec()], vec![group.to_vec()])?;
    let post_words = match expand_word(postamble)? {
        Some(words) => words,
        None => vec![postamble.to_vec()],
    };
    Ok(Some(cross(base, post_words)?))
}
