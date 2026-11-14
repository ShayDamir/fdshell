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

// mutants: -C --lib -C --test -C printf -- -E 'test(/^child::printf::/) | binary(printf)'

mod render_loop;
mod sink;

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};

use super::Ctx;
use render_loop::render;
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

mod conv;
mod conv_q;
mod conv_str;
mod escapes;
mod fmt_exp;
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
