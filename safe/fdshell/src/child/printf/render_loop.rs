//! The render loop of `printf`: one round over the format string per reuse,
//! its escape and specifier arms, and the error it stops on.

// mutants: -C --lib -C --test -C printf -- -E 'test(/^child::printf::/) | binary(printf)'

use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::Report;

use super::{Sink, conv, escapes, spec};

/// Render `fmt` against `args` into `sink`. The format string is reused while
/// arguments remain, matching bash; a round that consumes nothing ends it, and
/// so does a `%b` argument ending in `\c` (no further output, exit status
/// unchanged).
pub(super) fn render(
    fmt: &[u8],
    args: &[&CStr],
    sink: &mut Sink,
) -> Result<(), Report<BuiltinError>> {
    let mut rest = args;
    let mut stopped = false;
    loop {
        let consumed = round(fmt, &mut rest, sink, &mut stopped)?;
        if stopped || rest.is_empty() || !consumed {
            return Ok(());
        }
    }
}

fn round(
    fmt: &[u8],
    rest: &mut &[&CStr],
    sink: &mut Sink,
    stopped: &mut bool,
) -> Result<bool, Report<BuiltinError>> {
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
                consumed = conv::apply_spec(&s, rest, sink, stopped)? || consumed;
                if *stopped {
                    return Ok(consumed);
                }
                i = j;
            }
        }
    }
    Ok(consumed)
}

fn invalid_format() -> Report<BuiltinError> {
    Report::new(BuiltinError::InvalidFormat)
}
