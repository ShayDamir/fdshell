use error_stack::{Report, ResultExt};
use std::io::Write as _;

use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

use collect::collect_targets;
use flags::SourceFd;
use flags::parse_flags;
use io::read_line;
use strip::strip_prefix;
use words::split_fields;

pub(crate) fn run_read(
    line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "read", cmdline)?;

    let (fd_source, max_bytes, prompt) = parse_flags(&cmdline.args)?;
    let targets = collect_targets(&cmdline.args)?;

    let prompt_text = prompt.unwrap_or(b"");
    if !prompt_text.is_empty() {
        std::io::stderr()
            .write_all(prompt_text)
            .change_context(ReadError::Io)
            .change_context(CmdError::Read)?;
    }

    let resolved_fd: Option<sys::LocalFd> = match &fd_source {
        SourceFd::FdVar(name) => {
            let state = cell.borrow().change_context(CmdError::Read)?;
            let var = ShortCStr::from_vec(name.clone())
                .change_context(ReadError::BadTarget)
                .change_context(CmdError::Read)?;
            Some(
                state
                    .fds
                    .get(&var)
                    .ok_or(ReadError::VarNotFound)
                    .change_context(CmdError::Read)?
                    .try_clone()
                    .change_context(CmdError::Read)?,
            )
        }
        _ => None,
    };

    let (data, eof) = read_line(&fd_source, resolved_fd.as_ref(), max_bytes)?;
    if data.is_empty() && eof {
        let mut state = cell.borrow_mut().change_context(CmdError::Read)?;
        state.last_status = WaitStatus::Exited(1);
        return Ok(true);
    }

    let fields = split_fields(&data, targets.len());

    let mut state = cell.borrow_mut().change_context(CmdError::Read)?;
    for (i, name) in targets.iter().enumerate() {
        let field = fields.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
        let var_name = strip_prefix(name);
        let s = ShortCStr::from_vec(field.to_vec())
            .change_context(ReadError::NulByte)
            .change_context(CmdError::Read)?;
        state.strings.insert(var_name, s);
    }
    state.last_status = WaitStatus::Exited(0);
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;

mod collect;
mod flags;
mod io;
mod read_from_fd;
mod strip;
mod words;
