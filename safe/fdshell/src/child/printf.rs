//! `printf FMT [ARG...]` — format-string output.
//!
//! POSIX/XCU conversion table: `%s %c %d %i %u %o %x %X %b %q %a %A %e %E
//! %f %F %g %G %%` with width, precision and flags. Numeric arguments use C
//! `strtol`/`strtod` prefix semantics; a numeric failure (junk after the parsed
//! prefix, or nothing parsed) prints a diagnostic to stderr, sets the exit
//! status, and the value is the parsed prefix (0 when nothing parsed), while an
//! out-of-range value is the clamped bound; a malformed format specifier stops
//! the render with an error.
//!
//! **Documented deviations** (f64-based, C locale):
//! - No positional arguments (`%1$s` is not supported).
//! - Floats are `f64`: out-of-range values render as `inf`/`0` with a range
//!   diagnostic (bash's bignum `%f` for `|x| > 1.8e308` is not reproduced).
//! - `%a`/`%A` mantissa digits are the `f64` value's, zero-extended to 15; for
//!   inputs whose decimal→binary mapping differs between `f64` and glibc's
//!   long double (e.g. `0.1`, `5e-324`) the trailing digits are the `f64`
//!   form, not bash's long-double digits.

mod sink;

use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};

use super::Ctx;
pub(super) use sink::Sink;

pub(super) fn handle_printf(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let mut sink = Sink::new();
    match ctx.refs.split_first() {
        Some((fmt, args)) => render(fmt.to_bytes(), args, &mut sink)?,
        // Bash `printf` with no arguments prints the default `%s\n` format.
        None => sink.out.push(b'\n'),
    }
    if !sink.err.is_empty() {
        sys::ERR
            .write_all(&sink.err)
            .change_context(BuiltinError::Io)?;
    }
    sys::OUT
        .write_all(&sink.out)
        .change_context(BuiltinError::Io)?;
    Ok(i32::from(sink.failed))
}

/// Render `fmt` against `args` into `sink`. The format string is reused while
/// arguments remain, matching bash; a round that consumes nothing ends it.
fn render(fmt: &[u8], args: &[&CStr], sink: &mut Sink) -> Result<(), Report<BuiltinError>> {
    let mut rest = args;
    loop {
        let consumed = round(fmt, &mut rest, sink)?;
        if rest.is_empty() || !consumed {
            return Ok(());
        }
    }
}

fn round(fmt: &[u8], rest: &mut &[&CStr], sink: &mut Sink) -> Result<bool, Report<BuiltinError>> {
    let mut consumed = false;
    let mut i = 0;
    while let Some(&b) = fmt.get(i) {
        if b != b'%' {
            if b == b'\\' {
                i = escapes::emit_escape(fmt, i, &mut sink.out);
                continue;
            }
            sink.out.push(b);
            i += 1;
            continue;
        }
        match fmt.get(i + 1).copied() {
            None => return Err(invalid_format()),
            Some(b'%') => {
                sink.out.push(b'%');
                i += 2;
            }
            Some(_) => {
                let (s, j) = spec::parse(fmt, i + 1).ok_or_else(invalid_format)?;
                consumed = conv::apply_spec(&s, rest, sink)? || consumed;
                i = j;
            }
        }
    }
    Ok(consumed)
}

fn invalid_format() -> Report<BuiltinError> {
    Report::new(BuiltinError::InvalidFormat)
}

mod conv;
mod conv_q;
mod conv_str;
mod escapes;
mod fmt_field;
mod fmt_float;
mod fmt_g;
mod fmt_hexfloat;
mod fmt_hexfloat_prec;
mod fmt_int;
mod spec;
mod spec_helper;

#[cfg(test)]
mod tests;
