//! Integer power for `$((…))`: `base ** exp`.

/// `base ** exp` with wrapping multiplication; a negative exponent is 0
/// (the fractional result truncates), `0 ** 0` is 1.
pub(super) fn powi(base: i64, exp: i64) -> i64 {
    if exp < 0 {
        return 0;
    }
    let mut result: i64 = 1;
    let mut b = base;
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.wrapping_mul(b);
        }
        e >>= 1;
        if e > 0 {
            b = b.wrapping_mul(b);
        }
    }
    result
}
