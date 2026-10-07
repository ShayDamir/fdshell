//! `g`/`G` conversion rendering: `f` or `e` form chosen by the exponent.

use alloc::string::String;
use alloc::vec::Vec;

use super::fmt_field::pad;
use super::fmt_float::{nonfinite, norm_e, sign_prefix, split_e};
use super::spec::Fmt;

/// Render a `g`/`G` conversion (the value is already parsed) into `out`.
pub(super) fn render(v: f64, fmt: &Fmt, out: &mut Vec<u8>) {
    let upper = fmt.conv == b'G';
    if let Some(word) = nonfinite(v, upper) {
        let (prefix, core) = split_leading(&word);
        pad(
            fmt.left,
            fmt.zero,
            fmt.width.unwrap_or(0),
            prefix,
            core,
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
    let prefix = sign_prefix(v, fmt);
    pad(
        fmt.left,
        fmt.zero,
        fmt.width.unwrap_or(0),
        prefix.as_bytes(),
        body.as_bytes(),
        out,
    );
}

fn fixed_with(v: f64, prec: usize) -> String {
    alloc::format!("{:.prec$}", v.abs())
}

fn exponent_with(v: f64, prec: usize, upper: bool) -> String {
    let s = norm_e(&alloc::format!("{:.prec$e}", v.abs()));
    if upper { s.to_uppercase() } else { s }
}

/// The base-10 exponent of `v` (the `E` in `d.ddd × 10^E`), from a full-
/// precision `%e` rendering so the mantissa cannot round across a power of 10.
fn exp10(v: f64) -> i32 {
    if v == 0.0 {
        return 0;
    }
    let s = alloc::format!("{:.17e}", v.abs());
    split_e(&s).map(|(_, e)| e).unwrap_or(0)
}

/// Strip trailing zeros (and a trailing dot) from the fractional part, which
/// ends at the first `e`/`E` or at the end of the string.
fn strip_trailing(s: &str) -> String {
    let bytes = s.as_bytes();
    let Some(dot) = bytes.iter().position(|&c| c == b'.') else {
        return String::from(s);
    };
    let end = bytes
        .iter()
        .position(|&c| c == b'e' || c == b'E')
        .unwrap_or(bytes.len());
    let mut last = None;
    for i in (dot + 1)..end {
        if bytes.get(i) != Some(&b'0') {
            last = Some(i);
        }
    }
    let keep = last.map(|i| i + 1).unwrap_or(dot);
    let mut out = bytes.get(..keep).unwrap_or(&[]).to_vec();
    if let Some(exp) = bytes.get(end..) {
        out.extend_from_slice(exp);
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Split a leading `-` off a rendered word into `(sign, rest)`.
fn split_leading(word: &str) -> (&[u8], &[u8]) {
    let b = word.as_bytes();
    match b.split_first() {
        Some((c, rest)) if *c == b'-' => (core::slice::from_ref(c), rest),
        _ => (&[], b),
    }
}
