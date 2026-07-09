use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use builtins::error::Suggestion;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

type ReadResult<T> = Result<T, Report<CmdError>>;

type ReadFlags<'a> = (SourceFd, Option<usize>, Option<&'a [u8]>);

pub(crate) fn parse_flags<'a>(args: &'a [ShortCStr]) -> ReadResult<ReadFlags<'a>> {
    let mut iter = args.iter();
    let mut source = SourceFd::Stdin;
    let mut max_bytes: Option<usize> = None;
    let mut prompt: Option<&[u8]> = None;

    while let Some(arg) = iter.next() {
        let bytes = arg.as_bytes().change_context(CmdError::Read)?;
        match bytes {
            b"-u" => {
                let fd_arg = iter
                    .next()
                    .ok_or(ReadError::MissingArgument('u'))
                    .change_context(CmdError::Read)?;
                let fd_bytes = fd_arg.as_bytes().change_context(CmdError::Read)?;
                if let Some(name) = fd_bytes.strip_prefix(b"%") {
                    source = SourceFd::FdVar(name.to_vec());
                } else if let Ok(n) = core::str::from_utf8(fd_bytes)
                    .change_context(ReadError::InvalidArgument('u'))
                    .change_context(CmdError::Read)?
                    .parse::<i32>()
                {
                    source = SourceFd::RawFd(n);
                } else {
                    return Err(Report::new(ReadError::InvalidArgument('u'))
                        .attach_opaque(Suggestion(
                            "-u value must be a number (e.g. 3) or a %variable",
                        ))
                        .change_context(CmdError::Read));
                }
            }
            b"-n" => {
                let n_arg = iter
                    .next()
                    .ok_or(ReadError::MissingArgument('n'))
                    .change_context(CmdError::Read)?;
                let n_bytes = n_arg.as_bytes().change_context(CmdError::Read)?;
                match core::str::from_utf8(n_bytes)
                    .change_context(ReadError::InvalidArgument('n'))
                    .change_context(CmdError::Read)?
                    .parse::<usize>()
                {
                    Ok(n) => max_bytes = Some(n),
                    Err(_) => {
                        return Err(Report::new(ReadError::InvalidArgument('n'))
                            .attach_opaque(Suggestion("-n value must be a non-negative integer"))
                            .change_context(CmdError::Read));
                    }
                }
            }
            b"-p" => {
                let p_arg = iter
                    .next()
                    .ok_or(ReadError::MissingArgument('p'))
                    .change_context(CmdError::Read)?;
                let p_bytes = p_arg.as_bytes().change_context(CmdError::Read)?;
                prompt = Some(p_bytes);
            }
            _ => {}
        }
    }

    Ok((source, max_bytes, prompt))
}

#[derive(Debug)]
pub(crate) enum SourceFd {
    Stdin,
    RawFd(i32),
    FdVar(Vec<u8>),
}
