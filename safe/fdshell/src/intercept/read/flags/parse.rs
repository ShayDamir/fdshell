// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use crate::numarg::{LEGAL_NUMBER, decimal, seconds_ms};
use builtins::error::Suggestion;
use core::slice::Iter;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::{ReadFlags, SourceFd};

const N_SUGGESTION: &str =
    "-n value must be a non-negative integer (surrounding spaces and a sign are allowed)";
const T_SUGGESTION: &str = "-t value must be a non-negative number of seconds, whole or \
                           fractional like 0.5 (a leading + is allowed)";

pub(crate) fn parse_flags<'a>(args: &'a [ShortCStr]) -> Result<ReadFlags<'a>, Report<CmdError>> {
    let mut iter = args.iter();
    let mut flags = ReadFlags {
        source: SourceFd::Stdin,
        max_bytes: None,
        prompt: None,
        raw: false,
        delim: None,
        timeout: None,
    };

    while let Some(arg) = iter.next() {
        let bytes = arg.as_bytes().change_context(CmdError::Read)?;
        match bytes {
            b"-u" => {
                let fd_arg = next_arg(&mut iter, 'u')?;
                flags.source = match fd_arg.strip_prefix(b"%") {
                    Some(name) => SourceFd::FdVar(name),
                    None => SourceFd::RawFd(fd_arg.clone()),
                };
            }
            b"-n" => {
                let n_arg = next_arg(&mut iter, 'n')?;
                let n_bytes = n_arg.as_bytes().change_context(CmdError::Read)?;
                let n = decimal(n_bytes, &LEGAL_NUMBER)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| reject('n', N_SUGGESTION))?;
                flags.max_bytes = Some(n);
            }
            b"-p" => flags.prompt = Some(bytes_arg(&mut iter, 'p')?),
            b"-r" => flags.raw = true,
            b"-d" => flags.delim = Some(bytes_arg(&mut iter, 'd')?),
            b"-t" => {
                let t_arg = next_arg(&mut iter, 't')?;
                let t_bytes = t_arg.as_bytes().change_context(CmdError::Read)?;
                flags.timeout = Some(seconds_ms(t_bytes).ok_or_else(|| reject('t', T_SUGGESTION))?);
            }
            _ => {}
        }
    }

    Ok(flags)
}

fn next_arg<'a>(
    iter: &mut Iter<'a, ShortCStr>,
    flag: char,
) -> Result<&'a ShortCStr, Report<CmdError>> {
    iter.next()
        .ok_or(ReadError::MissingArgument(flag))
        .change_context(CmdError::Read)
}

/// The byte sequence of the next flag's argument (the raw value flags `-p`/`-d`).
fn bytes_arg<'a>(iter: &mut Iter<'a, ShortCStr>, flag: char) -> Result<&'a [u8], Report<CmdError>> {
    next_arg(iter, flag)?
        .as_bytes()
        .change_context(CmdError::Read)
}

/// The flag's rejection report; the value grammars live in `crate::numarg`
/// and each flag names the one bash uses for it.
fn reject(flag: char, suggestion: &'static str) -> Report<CmdError> {
    Report::new(ReadError::InvalidArgument(flag))
        .attach_opaque(Suggestion(suggestion))
        .change_context(CmdError::Read)
}
