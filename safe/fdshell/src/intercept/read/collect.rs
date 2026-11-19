// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

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
            b"-u" | b"-n" | b"-p" | b"-d" | b"-t" => {
                iter.next();
            }
            b"-r" => {}
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

    // An empty list is a valid answer: bash's no-name `read` fills REPLY,
    // and `run_read` keeps the "no names" signal this return provides.
    Ok(targets)
}
