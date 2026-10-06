//! `g`/`G` conversion rendering: `f` or `e` form chosen by the exponent.

use alloc::string::String;
use alloc::vec::Vec;

use super::fmt_field::pad;
use super::fmt_float::{apply_sign, nonfinite, norm_e, split_e};
use super::spec::Fmt;

/// Render a `g`/`G` conversion (the value is already parsed) into `out`.
pub(super) fn render(v: f64, fmt: &Fmt, out: &mut Vec<u8>) {
    let upper = fmt.conv == b'G';
    if let Some(s) = nonfinite(v, upper) {
        pad(
            fmt.left,
            fmt.zero,
            fmt.width.unwrap_or(0),
            s.as_bytes(),
            out,
        );
        return;
    }
    let p = match fmt.precision {
        None => 6,
        Some(0) => 1,
        Some(p) => p,
    };
    let e = exp10(v);
    let body = if (p as i64) > e as i64 && e >= -4 {
        let prec = ((p as i64) - 1 - e as i64) as usize;
        fixed_with(v, prec)
    } else {
        exponent_with(v, p - 1, upper)
    };
    let body = if fmt.alt { body } else { strip_trailing(&body) };
    let s = apply_sign(body, v, fmt);
    pad(
        fmt.left,
        fmt.zero,
        fmt.width.unwrap_or(0),
        s.as_bytes(),
        out,
    );
}

fn fixed_with(v: f64, prec: usize) -> String {
    alloc::format!("{v:.prec$}")
}

fn exponent_with(v: f64, prec: usize, upper: bool) -> String {
    let s = norm_e(&alloc::format!("{v:.prec$e}"));
    if upper { s.to_uppercase() } else { s }
}

/// The base-10 exponent of `v` (the `E` in `d.ddd × 10^E`), from a full-
/// precision `%e` rendering so the mantissa cannot round across a power of 10.
fn exp10(v: f64) -> i32 {
    if v == 0.0 {
        return 0;
    }
    let s = alloc::format!("{v:.17e}");
    split_e(&s).map(|(_, e)| e).unwrap_or(0)
}

/// Strip trailing zeros (and a trailing dot) from the fractional part, which
/// ends at the first `e` or at the end of the string.
fn strip_trailing(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let Some(dot) = chars.iter().position(|&c| c == '.') else {
        return String::from(s);
    };
    let end = chars.iter().position(|&c| c == 'e').unwrap_or(chars.len());
    let mut last = None;
    for i in (dot + 1)..end {
        if chars.get(i) != Some(&'0') {
            last = Some(i);
        }
    }
    let keep = last.map(|i| i + 1).unwrap_or(dot);
    let mut out: Vec<char> = chars.get(..keep).map(Vec::from).unwrap_or_default();
    if let Some(exp) = chars.get(end..) {
        out.extend_from_slice(exp);
    }
    out.into_iter().collect()
}
