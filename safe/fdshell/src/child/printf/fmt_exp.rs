//! The `%e` rendering helpers shared by the `e`/`E` and `g`/`G` formatters.

use alloc::string::String;

/// `d.dddde±dd`: format the magnitude in Rust's `%e` form, then normalise the
/// exponent to the signed, at-least-two-digit glibc style; `%E` upper-cases.
pub(super) fn exponent(v: f64, prec: usize, upper: bool) -> String {
    let s = norm_e(&alloc::format!("{:.prec$e}", v.abs()));
    if upper { s.to_uppercase() } else { s }
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
