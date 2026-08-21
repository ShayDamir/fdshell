//! Conversion specifiers of `printf`.

use alloc::vec::Vec;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use core::str::FromStr;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(super) fn is_conv(c: u8) -> bool {
    matches!(c, b's' | b'd' | b'i' | b'u' | b'o' | b'x' | b'X' | b'c')
}

/// Apply one conversion; returns `true` when an argument was consumed.
pub(super) fn apply_conv(
    c: u8,
    rest: &mut &[&CStr],
    out: &mut Vec<u8>,
) -> Result<bool, Report<BuiltinError>> {
    match c {
        b's' | b'c' => string_conv(c, rest, out),
        b'd' | b'i' => num_conv::<i64>(rest, out, |v| alloc::format!("{v}")),
        b'u' => num_conv::<i64>(rest, out, |v| alloc::format!("{}", v as u64)),
        b'o' => num_conv::<i64>(rest, out, |v| alloc::format!("{:o}", v as u64)),
        b'x' => num_conv::<i64>(rest, out, |v| alloc::format!("{:x}", v as u64)),
        b'X' => num_conv::<i64>(rest, out, |v| alloc::format!("{:X}", v as u64)),
        // `is_conv` guarantees a known conversion character.
        _ => Err(Report::new(BuiltinError::Never)),
    }
}

/// `%s` / `%c` take one argument, or print nothing when the arguments are
/// exhausted (bash semantics).
fn string_conv(
    c: u8,
    rest: &mut &[&CStr],
    out: &mut Vec<u8>,
) -> Result<bool, Report<BuiltinError>> {
    match rest.split_first() {
        Some((a, tail)) => {
            *rest = tail;
            match c {
                b'c' => {
                    if let Some(b) = a.to_bytes().first() {
                        out.push(*b);
                    }
                }
                _ => out.extend_from_slice(a.to_bytes()),
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Numeric conversions take one argument, or `0` when the arguments are
/// exhausted (bash semantics).
fn num_conv<T>(
    rest: &mut &[&CStr],
    out: &mut Vec<u8>,
    f: impl FnOnce(T) -> alloc::string::String,
) -> Result<bool, Report<BuiltinError>>
where
    T: FromStr + Default,
    T::Err: core::error::Error + Send + Sync + 'static,
{
    let v = take(rest)?;
    out.extend_from_slice(f(v).as_bytes());
    Ok(true)
}

fn take<T>(rest: &mut &[&CStr]) -> Result<T, Report<BuiltinError>>
where
    T: FromStr + Default,
    T::Err: core::error::Error + Send + Sync + 'static,
{
    match rest.split_first() {
        Some((a, tail)) => {
            *rest = tail;
            let mut s = ShortCStr::new();
            s.push(a);
            s.parse::<T>()
                .change_context(BuiltinError::InvalidArgument("number"))
        }
        None => Ok(T::default()),
    }
}
