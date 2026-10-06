//! The string-taking conversions (`s c b q`): they consume one argument (or
//! print an empty value when the arguments are exhausted) and pad to the width.

use alloc::vec::Vec;
use core::ffi::CStr;

use super::conv::next_arg_bytes;
use super::conv_q::render as quote;
use super::escapes::emit_arg;
use super::fmt_field::pad;
use super::spec::Fmt;

/// The next argument's bytes (empty when exhausted) and whether one was taken.
fn arg_bytes(rest: &mut &[&CStr]) -> (Vec<u8>, bool) {
    match next_arg_bytes(rest) {
        Some(a) => (a, true),
        None => (Vec::new(), false),
    }
}

/// `%s`: the argument bytes, truncated to the precision, space-padded.
pub(super) fn string_conv(rest: &mut &[&CStr], fmt: &Fmt, out: &mut Vec<u8>) {
    let (mut b, _) = arg_bytes(rest);
    if let Some(p) = fmt.precision {
        b.truncate(p);
    }
    pad(fmt.left, false, fmt.width.unwrap_or(0), &b, out);
}

/// `%c`: the argument's first byte (NUL when empty or exhausted), padded.
pub(super) fn char_conv(rest: &mut &[&CStr], fmt: &Fmt, out: &mut Vec<u8>) {
    let (b, _) = arg_bytes(rest);
    let c = b.first().copied().unwrap_or(0);
    pad(
        fmt.left,
        false,
        fmt.width.unwrap_or(0),
        core::slice::from_ref(&c),
        out,
    );
}

/// `%b`: the argument with backslash escapes expanded (the precision truncates
/// the raw bytes first), then padded.
pub(super) fn byte_conv(rest: &mut &[&CStr], fmt: &Fmt, out: &mut Vec<u8>) {
    let (mut b, _) = arg_bytes(rest);
    if let Some(p) = fmt.precision {
        b.truncate(p);
    }
    let mut core = Vec::new();
    emit_arg(&b, &mut core);
    pad(fmt.left, false, fmt.width.unwrap_or(0), &core, out);
}

/// `%q`: the argument shell-quoted (the precision truncates the raw bytes
/// first), then padded.
pub(super) fn quote_conv(rest: &mut &[&CStr], fmt: &Fmt, out: &mut Vec<u8>) {
    let (mut b, _) = arg_bytes(rest);
    if let Some(p) = fmt.precision {
        b.truncate(p);
    }
    let core = quote(&b);
    pad(fmt.left, false, fmt.width.unwrap_or(0), &core, out);
}
