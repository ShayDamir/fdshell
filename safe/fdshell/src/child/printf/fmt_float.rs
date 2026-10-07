//! Float conversion rendering (`f F e E`) and the helpers shared with `g`.

use alloc::string::String;
use alloc::vec::Vec;

use super::fmt_field::pad;
use super::spec::Fmt;

/// Render a `f`/`F`/`e`/`E` conversion (the value is already parsed) into
/// `out`.
pub(super) fn render(v: f64, fmt: &Fmt, out: &mut Vec<u8>) {
    let upper = matches!(fmt.conv, b'F' | b'E');
    if let Some(word) = nonfinite(v, upper) {
        let (prefix, core) = split_sign(&word);
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
    let core = match fmt.conv {
        b'f' | b'F' => fixed(v, fmt),
        b'e' | b'E' => exponent(v, fmt, upper),
        // The dispatcher only calls this for these conversions.
        _ => String::new(),
    };
    let prefix = sign_prefix(v, fmt);
    pad(
        fmt.left,
        fmt.zero,
        fmt.width.unwrap_or(0),
        prefix.as_bytes(),
        core.as_bytes(),
        out,
    );
}

/// `d.ddd` with the given precision (default 6); `#` keeps the dot at
/// precision 0. The value is rendered in its magnitude (the sign is separate).
fn fixed(v: f64, fmt: &Fmt) -> String {
    let prec = fmt.precision.unwrap_or(6);
    let mut s = alloc::format!("{:.prec$}", v.abs());
    if fmt.alt && prec == 0 {
        s.push('.');
    }
    s
}

/// `d.dddde±dd`: format the magnitude in Rust's `%e` form, then normalise the
/// exponent to the signed, at-least-two-digit glibc style; `%E` upper-cases.
fn exponent(v: f64, fmt: &Fmt, upper: bool) -> String {
    let prec = fmt.precision.unwrap_or(6);
    let s = norm_e(&alloc::format!("{:.prec$e}", v.abs()));
    if upper { s.to_uppercase() } else { s }
}

/// The `inf`/`nan` rendering (case follows the conversion letter); a negative
/// value carries a `-`.
pub(super) fn nonfinite(v: f64, upper: bool) -> Option<String> {
    if v.is_nan() {
        return Some(String::from(if upper { "NAN" } else { "nan" }));
    }
    if v.is_infinite() {
        let sign = if v.is_sign_negative() { "-" } else { "" };
        let word = if upper { "INF" } else { "inf" };
        return Some(alloc::format!("{sign}{word}"));
    }
    None
}

/// The sign prefix for a value: `-` for negative (including `-0`), `+`/` ` for
/// the flag on a non-negative value, else empty.
pub(super) fn sign_prefix(v: f64, fmt: &Fmt) -> &'static str {
    if v.is_sign_negative() {
        "-"
    } else if fmt.plus {
        "+"
    } else if fmt.space {
        " "
    } else {
        ""
    }
}

/// Split a leading `-` off a rendered word into `(sign, rest)`.
fn split_sign(word: &str) -> (&[u8], &[u8]) {
    let b = word.as_bytes();
    match b.split_first() {
        Some((c, rest)) if *c == b'-' => (core::slice::from_ref(c), rest),
        _ => (&[], b),
    }
}

/// Split a Rust `%e` string into its mantissa and base-10 exponent.
pub(super) fn split_e(s: &str) -> Option<(&str, i32)> {
    let e_pos = s.find('e')?;
    let mantissa = s.get(..e_pos)?;
    let e: i32 = s.get(e_pos + 1..)?.parse().ok()?;
    Some((mantissa, e))
}

/// Reformat a Rust `%e` string to the glibc style (signed, ≥2-digit exponent).
pub(super) fn norm_e(s: &str) -> String {
    match split_e(s) {
        Some((mantissa, e)) => {
            let sign = if e < 0 { "-" } else { "+" };
            alloc::format!("{mantissa}e{sign}{:02}", e.abs())
        }
        None => String::from(s),
    }
}
