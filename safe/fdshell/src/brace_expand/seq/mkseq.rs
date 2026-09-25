//! Element generation for a valid [SeqSpec] (port of bash's `mkseq`).

use super::{MAX_WORDS, SeqKind, SeqSpec};
use crate::error::parse::ParseError;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, bail};

/// The walk step after bash's sign rules: `0` becomes `1`, and the sign is
/// flipped when it disagrees with the `start`..`end` direction. `None` on
/// `i64::MIN` overflow (bash: the sequence is invalid).
fn effective_incr(spec: &SeqSpec) -> Option<i64> {
    let mut incr = spec.incr;
    if incr == 0 {
        incr = 1;
    }
    if (spec.start < spec.end) == (incr < 0) {
        let flipped = incr.checked_neg()?;
        incr = flipped;
    }
    Some(incr)
}

/// How many elements the sequence produces, enforcing the [MAX_WORDS] cap.
/// `Ok(None)` on arithmetic overflow (bash: the sequence is invalid, the word
/// stays literal); `Err` when the count exceeds the cap.
pub fn element_count(spec: &SeqSpec) -> Result<Option<usize>, Report<ParseError>> {
    let Some(incr) = effective_incr(spec) else {
        return Ok(None);
    };
    count_of(spec, incr)
}

/// The element count for a known walk step `incr`, enforcing the [MAX_WORDS]
/// cap. `Ok(None)` on arithmetic overflow; `Err` when the count exceeds the
/// cap (including `count` itself overflowing `i64`).
fn count_of(spec: &SeqSpec, incr: i64) -> Result<Option<usize>, Report<ParseError>> {
    // bash's `abs_incr = -incr` is checked arithmetic: on `i64::MIN` it
    // overflows and the sequence is invalid. `unsigned_abs` wraps that value
    // to `u64::MAX`, which `i64::try_from` rejects.
    let Some(abs_incr) = i64::try_from(incr.unsigned_abs()).ok() else {
        return Ok(None);
    };
    // The element span is `|end - start|`; `None` when the gap itself
    // overflows `i64` (bash's checked arithmetic fails there too, so the
    // sequence is invalid).
    let Some(prevn) = spec
        .end
        .checked_sub(spec.start)
        .and_then(|d| i64::try_from(d.unsigned_abs()).ok())
    else {
        return Ok(None);
    };
    let Some(count) = (prevn / abs_incr).checked_add(1) else {
        bail!(ParseError::BraceExpansionTooManyWords);
    };
    if count > MAX_WORDS as i64 {
        bail!(ParseError::BraceExpansionTooManyWords);
    }
    Ok(Some(count as usize))
}

/// Generate the elements of the sequence. `Ok(None)` on arithmetic overflow
/// (bash: the sequence is invalid, the word stays literal); `Err` when the
/// element count exceeds [MAX_WORDS].
pub fn mkseq(spec: &SeqSpec) -> Result<Option<Vec<Vec<u8>>>, Report<ParseError>> {
    let Some(incr) = effective_incr(spec) else {
        return Ok(None);
    };
    let Some(count) = count_of(spec, incr)? else {
        return Ok(None);
    };
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(count);
    let mut n = spec.start;
    for _ in 0..count {
        out.push(render(n, spec.kind, spec.width));
        // The walk stays within `start..=end`; the final increment is
        // discarded, so the wrapping never observes an overflow.
        n = n.wrapping_add(incr);
    }
    Ok(Some(out))
}

/// Render one element (bash's `itos` plus ST_ZINT zero-padding).
fn render(n: i64, kind: SeqKind, width: usize) -> Vec<u8> {
    match kind {
        SeqKind::Char => vec![n as u8],
        SeqKind::Int => alloc::format!("{n}").into_bytes(),
        SeqKind::ZInt => {
            let text = alloc::format!("{n}");
            if text.len() >= width {
                return text.into_bytes();
            }
            let mut out: Vec<u8> = Vec::with_capacity(width);
            if n < 0 {
                out.push(b'-');
                let digits = alloc::format!("{}", n.unsigned_abs());
                out.extend(core::iter::repeat_n(b'0', width - out.len() - digits.len()));
                out.extend_from_slice(digits.as_bytes());
            } else {
                out.extend(core::iter::repeat_n(b'0', width - text.len()));
                out.extend_from_slice(text.as_bytes());
            }
            out
        }
    }
}
