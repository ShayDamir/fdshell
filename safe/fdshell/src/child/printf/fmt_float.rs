//! Float conversion rendering (`f F e E`) and the helpers shared with `g`.

use alloc::string::String;
use alloc::vec::Vec;

use super::fmt_field::pad;
use super::spec::Fmt;

/// Render a `f`/`F`/`e`/`E` conversion (the value is already parsed) into
/// `out`.
pub(super) fn render(v: f64, fmt: &Fmt, out: &mut Vec<u8>) {
    let upper = matches!(fmt.conv, b'F' | b'E');
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
    let s = match fmt.conv {
        b'f' | b'F' => fixed(v, fmt),
        b'e' | b'E' => exponent(v, fmt),
        // The dispatcher only calls this for these conversions.
        _ => String::new(),
    };
    let s = apply_sign(s, v, fmt);
    pad(
        fmt.left,
        fmt.zero,
        fmt.width.unwrap_or(0),
        s.as_bytes(),
        out,
    );
}

/// `d.ddd` with the given precision (default 6); `#` keeps the dot at
/// precision 0.
fn fixed(v: f64, fmt: &Fmt) -> String {
    let prec = fmt.precision.unwrap_or(6);
    let mut s = alloc::format!("{v:.prec$}");
    if fmt.alt && prec == 0 {
        s.push('.');
    }
    s
}

/// `d.dddde±dd`: format in Rust's `%e` form, then normalise the exponent to
/// the signed, at-least-two-digit glibc style.
fn exponent(v: f64, fmt: &Fmt) -> String {
    let prec = fmt.precision.unwrap_or(6);
    norm_e(&alloc::format!("{v:.prec$e}"))
}

/// The `inf`/`nan` rendering (case follows the conversion letter).
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

/// Prepend the `+`/` ` sign flag for non-negative values (a negative value,
/// including `-0`, already carries its `-`).
pub(super) fn apply_sign(s: String, v: f64, fmt: &Fmt) -> String {
    if v.is_sign_negative() || (!fmt.plus && !fmt.space) {
        return s;
    }
    let mut r = String::with_capacity(s.len() + 1);
    r.push(if fmt.plus { '+' } else { ' ' });
    r.push_str(&s);
    r
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
