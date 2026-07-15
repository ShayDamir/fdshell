use alloc::vec::Vec;
use builtins::error::Suggestion;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use sys::ShortCStr;

pub(crate) fn collect_targets(args: &[ShortCStr]) -> Result<Vec<ShortCStr>, Report<CmdError>> {
    let mut targets = Vec::new();
    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        let bytes = arg.as_bytes().change_context(CmdError::Read)?;
        match bytes {
            b"-u" | b"-n" | b"-p" => {
                iter.next();
            }
            _ => {
                if bytes.starts_with(b"%") {
                    return Err(Report::new(ReadError::FdVarUnsupported)
                        .attach_opaque(Suggestion(
                            "use a bare name like 'read var' for string variables",
                        ))
                        .change_context(CmdError::Read));
                }
                targets.push(arg.clone());
            }
        }
    }

    if targets.is_empty() {
        return Err(Report::new(ReadError::NoTarget)
            .attach_opaque(Suggestion(
                "usage: read [-u fd] [-n count] [-p prompt] var ...",
            ))
            .change_context(CmdError::Read));
    }

    Ok(targets)
}
