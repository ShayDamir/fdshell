//! The specifier parse loop: flags, width, precision, conversion character.

use super::super::spec_helper::{apply_flag, assign_field, digits, is_flag};
use super::{Field, Spec, is_conv};

/// Parse a specifier starting at the byte after `%`. Returns the spec and the
/// index just past the conversion character, or `None` when the bytes do not
/// form a valid specifier.
pub fn parse(fmt: &[u8], mut i: usize) -> Option<(Spec, usize)> {
    let mut s = Spec::initial();
    let mut in_precision = false;
    while let Some(&c) = fmt.get(i) {
        let in_fields = in_precision || s.width.is_some() || s.precision.is_some();
        if !in_fields && is_flag(c) {
            apply_flag(&mut s, c);
            i += 1;
            continue;
        }
        match c {
            b'.' => in_precision = true,
            b'*' => {
                if !assign_field(&mut s, in_precision, Field::Star) {
                    return None;
                }
            }
            c if c.is_ascii_digit() => {
                let (v, j) = digits(fmt, i)?;
                if !assign_field(&mut s, in_precision, Field::Lit(v)) {
                    return None;
                }
                i = j;
                continue;
            }
            c if is_conv(c) => {
                if in_precision && s.precision.is_none() {
                    s.precision = Some(Field::Lit(0));
                }
                s.conv = c;
                return Some((s, i + 1));
            }
            _ => return None,
        }
        i += 1;
    }
    None
}
