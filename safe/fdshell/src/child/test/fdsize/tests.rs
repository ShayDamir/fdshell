use super::{Fdsize, parse};

#[test]
fn parses_both_signs() {
    assert_eq!(parse(b"-fdsize+12"), Some(Fdsize::AtLeast(12)));
    assert_eq!(parse(b"-fdsize-12"), Some(Fdsize::AtMost(12)));
}

#[test]
fn parses_zero_bound() {
    assert_eq!(parse(b"-fdsize+0"), Some(Fdsize::AtLeast(0)));
    assert_eq!(parse(b"-fdsize-0"), Some(Fdsize::AtMost(0)));
}

#[test]
fn parses_u64_max() {
    assert_eq!(
        parse(b"-fdsize+18446744073709551615"),
        Some(Fdsize::AtLeast(u64::MAX))
    );
}

#[test]
fn rejects_malformed_ops() {
    let ops: &[&[u8]] = &[
        b"-fdsize",
        b"-fdsize+",
        b"-fdsize-",
        b"-fdsize5",                     // no sign
        b"-fdsize+x",                    // non-digit
        b"-fdsize+1x",                   // trailing junk
        b"-fdsize++1",                   // double sign
        b"-fdsize+18446744073709551616", // u64 overflow
        b"-fdsize+-1",                   // negative bound
        b"-e",                           // unrelated operator
        b"",                             // empty
    ];
    for op in ops {
        assert_eq!(parse(op), None, "{op:?}");
    }
}
