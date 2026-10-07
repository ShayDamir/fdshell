//! Conversion dispatch: resolve the fields, parse the argument, and route to
//! the matching formatter. Numeric failures are reported to the `Sink` (the
//! value falls back to 0 or the clamped bound) rather than aborting.

mod num;

use alloc::vec::Vec;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::Report;

use super::Sink;
use super::conv_str::{byte_conv, char_conv, quote_conv, string_conv};
use super::fmt_float;
use super::fmt_g;
use super::fmt_hexfloat;
use super::fmt_int;
use super::spec::{Field, Fmt, Spec};

/// Apply one fully-parsed specifier, consuming its `*` fields and its argument.
/// Returns `true` when at least one argument was consumed.
pub(super) fn apply_spec(
    spec: &Spec,
    rest: &mut &[&CStr],
    sink: &mut Sink,
) -> Result<bool, Report<BuiltinError>> {
    let before = rest.len();
    let (width, width_left) = resolve_field(spec.width, rest, sink, false);
    let (precision, _) = resolve_field(spec.precision, rest, sink, true);
    let fmt = Fmt {
        conv: spec.conv,
        left: spec.left || width_left,
        plus: spec.plus,
        space: spec.space,
        alt: spec.alt,
        zero: spec.zero,
        width,
        precision,
    };
    match spec.conv {
        b'd' | b'i' => fmt_int::render_signed(num::next_int(rest, sink), &fmt, &mut sink.out),
        b'u' | b'o' | b'x' | b'X' => {
            fmt_int::render_unsigned(num::next_uint(rest, sink), &fmt, &mut sink.out)
        }
        b'f' | b'F' | b'e' | b'E' => {
            fmt_float::render(num::next_float(rest, sink), &fmt, &mut sink.out)
        }
        b'g' | b'G' => fmt_g::render(num::next_float(rest, sink), &fmt, &mut sink.out),
        b'a' | b'A' => fmt_hexfloat::render(num::next_float(rest, sink), &fmt, &mut sink.out),
        b's' => string_conv(rest, &fmt, &mut sink.out),
        b'c' => char_conv(rest, &fmt, &mut sink.out),
        b'b' => byte_conv(rest, &fmt, &mut sink.out),
        b'q' => quote_conv(rest, &fmt, &mut sink.out),
        // `spec::is_conv` guarantees a known conversion character.
        _ => {}
    }
    Ok(before != rest.len())
}

/// Resolve a width/precision field to its value and whether a negative `*`
/// width forced left-justification. A negative `*` precision means "absent".
fn resolve_field(
    field: Option<Field>,
    rest: &mut &[&CStr],
    sink: &mut Sink,
    is_precision: bool,
) -> (Option<usize>, bool) {
    let Some(f) = field else {
        return (None, false);
    };
    match f {
        Field::Lit(n) => (Some(n), false),
        Field::Star => {
            let v = num::next_int(rest, sink);
            if is_precision {
                if v < 0 {
                    (None, false)
                } else {
                    (Some(v as usize), false)
                }
            } else if v < 0 {
                // A negative `*` width left-justifies at `|width|`.
                (Some(v.unsigned_abs() as usize), true)
            } else {
                (Some(v as usize), false)
            }
        }
    }
}

/// The next argument's bytes (owned), consumed from `rest`; `None` when the
/// arguments are exhausted.
pub(super) fn next_arg_bytes(rest: &mut &[&CStr]) -> Option<Vec<u8>> {
    let a = rest.first().copied()?;
    *rest = rest.get(1..).unwrap_or(&[]);
    Some(a.to_bytes().to_vec())
}
