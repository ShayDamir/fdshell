use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use builtins::error::Suggestion;
use core::slice::Iter;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::{ReadFlags, SourceFd};

const N_SUGGESTION: &str = "-n value must be a non-negative integer";
const T_SUGGESTION: &str = "-t value must be a non-negative integer (seconds)";

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
                flags.max_bytes = Some(parse_num(n_arg, 'n', N_SUGGESTION)?);
            }
            b"-p" => flags.prompt = Some(bytes_arg(&mut iter, 'p')?),
            b"-r" => flags.raw = true,
            b"-d" => flags.delim = Some(bytes_arg(&mut iter, 'd')?),
            b"-t" => {
                let t_arg = next_arg(&mut iter, 't')?;
                flags.timeout = Some(parse_num(t_arg, 't', T_SUGGESTION)?);
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

fn parse_num<T>(
    arg: &ShortCStr,
    flag: char,
    suggestion: &'static str,
) -> Result<T, Report<CmdError>>
where
    T: core::str::FromStr,
    T::Err: core::error::Error + Send + Sync + 'static,
{
    match arg.parse::<T>() {
        Ok(n) => Ok(n),
        Err(_) => Err(Report::new(ReadError::InvalidArgument(flag))
            .attach_opaque(Suggestion(suggestion))
            .change_context(CmdError::Read)),
    }
}
