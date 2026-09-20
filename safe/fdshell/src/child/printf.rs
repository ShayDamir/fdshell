//! `printf FMT [ARG...]` — format-string output.
//!
//! Bash-compatible subset: `%s %d %i %u %o %x %X %c %%` plus backslash
//! escapes in the format string. Width, precision, flags and `%b` are
//! unsupported (printed as-is). Numeric arguments are plain decimal
//! integers (no `0x`, no surrounding whitespace).

use crate::state::ShellState;
use alloc::vec::Vec;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(super) fn handle_printf(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let mut out = Vec::new();
    match refs.split_first() {
        Some((fmt, args)) => render(fmt.to_bytes(), args, &mut out)?,
        // Bash `printf` with no arguments prints the default `%s\n` format.
        None => out.push(b'\n'),
    }
    sys::OUT.write_all(&out).change_context(BuiltinError::Io)?;
    Ok(0)
}

/// Render `fmt` against `args` into `out`. The format string is reused while
/// arguments remain, matching bash; a round that consumes nothing ends it.
pub(super) fn render(
    fmt: &[u8],
    args: &[&CStr],
    out: &mut Vec<u8>,
) -> Result<(), Report<BuiltinError>> {
    let mut rest = args;
    loop {
        let consumed = round(fmt, &mut rest, out)?;
        if rest.is_empty() || !consumed {
            return Ok(());
        }
    }
}

fn round(fmt: &[u8], rest: &mut &[&CStr], out: &mut Vec<u8>) -> Result<bool, Report<BuiltinError>> {
    let mut consumed = false;
    let mut i = 0;
    while let Some(&b) = fmt.get(i) {
        if b != b'%' {
            if b == b'\\' {
                i = escapes::emit_escape(fmt, i, out);
                continue;
            }
            out.push(b);
            i += 1;
            continue;
        }
        match fmt.get(i + 1).copied() {
            None => out.push(b'%'),
            Some(b'%') => {
                out.push(b'%');
                i += 1;
            }
            Some(c) if conv::is_conv(c) => {
                consumed = conv::apply_conv(c, rest, out)? || consumed;
                i += 1;
            }
            Some(c) => {
                out.push(b'%');
                out.push(c);
                i += 1;
            }
        }
        i += 1;
    }
    Ok(consumed)
}

mod conv;
mod escapes;

#[cfg(test)]
mod tests;
