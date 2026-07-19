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
                if let Some(name) = fd_arg.strip_prefix(b"%") {
                    source = SourceFd::FdVar(name);
                } else {
                    source = SourceFd::RawFd(fd_arg.clone());
                }
            }
            b"-n" => {
                let n_arg = iter
                    .next()
                    .ok_or(ReadError::MissingArgument('n'))
                    .change_context(CmdError::Read)?;
                match n_arg.parse::<usize>() {
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
    RawFd(ShortCStr),
    FdVar(ShortCStr),
}
