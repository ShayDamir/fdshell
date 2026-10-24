//! The `%q` unit decoder: the argument split into the longest valid UTF-8
//! characters at each step, each invalid byte kept as a unit of its own.

use alloc::vec::Vec;

/// One decoded unit: a valid UTF-8 character, or a single invalid byte.
#[derive(Clone, Copy)]
pub enum Unit {
    Char(char),
    Byte(u8),
}

/// Decode `bytes` into the units `%q` quotes, one per character or bad byte.
pub fn decode(bytes: &[u8]) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        let mut decoded = false;
        for len in 1..=4usize {
            if let Some(s) = bytes
                .get(i..i + len)
                .and_then(|slice| core::str::from_utf8(slice).ok())
            {
                let mut it = s.chars();
                if let (Some(c), None) = (it.next(), it.next()) {
                    units.push(Unit::Char(c));
                    i += len;
                    decoded = true;
                    break;
                }
            }
        }
        if !decoded {
            units.push(Unit::Byte(b));
            i += 1;
        }
    }
    units
}
