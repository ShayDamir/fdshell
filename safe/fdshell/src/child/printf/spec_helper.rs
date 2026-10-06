//! Flag/field parsing helpers for [`super::spec`].

use super::spec::{Field, MAX_FIELD, Spec};

/// The flag characters that may appear before the fields.
pub(super) fn is_flag(c: u8) -> bool {
    matches!(c, b'-' | b'+' | b' ' | b'#' | b'0')
}

pub(super) fn apply_flag(s: &mut Spec, c: u8) {
    match c {
        b'-' => s.left = true,
        b'+' => s.plus = true,
        b' ' => s.space = true,
        b'#' => s.alt = true,
        b'0' => s.zero = true,
        _ => {}
    }
}

/// Store a field in the width or precision slot; `false` when that slot is
/// already filled (a second field is a format error).
pub(super) fn assign_field(s: &mut Spec, in_precision: bool, f: Field) -> bool {
    let slot = if in_precision {
        &mut s.precision
    } else {
        &mut s.width
    };
    if slot.is_some() {
        return false;
    }
    *slot = Some(f);
    true
}

/// Read the run of ASCII digits at `i` (there is at least one); the value is
/// capped at [`MAX_FIELD`]. Returns the value and the index past the digits.
pub(super) fn digits(fmt: &[u8], i: usize) -> Option<(usize, usize)> {
    let mut v = 0usize;
    let mut j = i;
    while let Some(&c) = fmt.get(j) {
        if !c.is_ascii_digit() {
            break;
        }
        v = (v * 10 + (c - b'0') as usize).min(MAX_FIELD);
        j += 1;
    }
    Some((v, j))
}
