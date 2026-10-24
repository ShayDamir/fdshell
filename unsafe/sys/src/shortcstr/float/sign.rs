//! The whitespace and sign prologue of `strtod`-style float parsing.

/// Skip leading C whitespace; the index of the first byte after it.
pub(super) fn skip_space(bytes: &[u8]) -> usize {
    let mut i = 0;
    while let Some(&c) = bytes.get(i) {
        if !matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
            break;
        }
        i += 1;
    }
    i
}

/// The `+`/`-` sign at `at`: the index just past it and whether it negates; a
/// missing sign leaves the cursor where it is.
pub(super) fn parse_sign(bytes: &[u8], at: usize) -> (usize, bool) {
    match bytes.get(at) {
        Some(b'-') => (at + 1, true),
        Some(b'+') => (at + 1, false),
        _ => (at, false),
    }
}
