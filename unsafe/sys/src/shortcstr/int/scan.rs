//! Scanning helpers for base-0 integer parsing: sign, base, and digit value.

/// Skip leading whitespace and an optional `+`/`-` sign.
pub(super) fn skip_sign(bytes: &[u8]) -> (bool, usize) {
    let mut i = 0;
    while let Some(&c) = bytes.get(i) {
        if !is_space(c) {
            break;
        }
        i += 1;
    }
    let mut neg = false;
    if let Some(&s) = bytes.get(i) {
        match s {
            b'+' => i += 1,
            b'-' => {
                neg = true;
                i += 1;
            }
            _ => {}
        }
    }
    (neg, i)
}

/// Select the base and the first digit position for a base-0 number.
pub(super) fn pick_base(rest: &[u8]) -> (u32, usize) {
    match (rest.first(), rest.get(1)) {
        (Some(b'0'), Some(b'x' | b'X')) => (16, 2),
        (Some(b'0'), Some(b'b' | b'B')) => (2, 2),
        (Some(b'0'), Some(_)) => (8, 0),
        _ => (10, 0),
    }
}

/// The value of `c` in `radix`, or `None` when it is not a digit.
pub(super) fn digit(c: u8, radix: u32) -> Option<u32> {
    let d = match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => return None,
    };
    ((d as u32) < radix).then_some(d as u32)
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}
