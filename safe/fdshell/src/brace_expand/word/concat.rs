//! The comma-amble splitting and the cartesian product (ports of bash's
//! `expand_amble` and `array_concat`).

use super::super::gobbler::gobble;
use super::super::seq::MAX_WORDS;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, bail};

/// Split the amble on top-level commas and expand each element recursively,
/// concatenating the results (bash's `expand_amble`).
pub(super) fn expand_amble(amble: &[u8]) -> Result<Vec<Vec<u8>>, Report<ParseError>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i <= amble.len() {
        let (ci, found, _) = gobble(amble, i, b',');
        let element = amble.get(start..ci).ok_or(ParseError::Never)?;
        match super::expand_word(element)? {
            Some(words) => out.extend(words),
            None => out.push(element.to_vec()),
        }
        if !found {
            break;
        }
        i = ci + 1;
        start = i;
    }
    Ok(out)
}

/// The output word count of a cartesian product, enforcing the [MAX_WORDS]
/// cap. `Err` when the product overflows `usize` or exceeds the cap.
pub fn cross_total(left: usize, right: usize) -> Result<usize, Report<ParseError>> {
    let Some(total) = left.checked_mul(right) else {
        bail!(ParseError::BraceExpansionTooManyWords);
    };
    if total > MAX_WORDS {
        bail!(ParseError::BraceExpansionTooManyWords);
    }
    Ok(total)
}

/// Cartesian product (bash's `array_concat`); a single empty word is the
/// identity. Bails when the result would exceed [MAX_WORDS].
pub fn cross(left: Vec<Vec<u8>>, right: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, Report<ParseError>> {
    if left.len() == 1 && left.first().is_some_and(|w| w.is_empty()) {
        return Ok(right);
    }
    if right.len() == 1 && right.first().is_some_and(|w| w.is_empty()) {
        return Ok(left);
    }
    let total = cross_total(left.len(), right.len())?;
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(total);
    for l in &left {
        for r in &right {
            let mut w: Vec<u8> = Vec::with_capacity(l.len() + r.len());
            w.extend_from_slice(l);
            w.extend_from_slice(r);
            out.push(w);
        }
    }
    Ok(out)
}
