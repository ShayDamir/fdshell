//! `read` builtin: reads one line from stdin, `-u fd`, or an fd var.
//!
//! `-t` is a one-shot `poll` gate (see `io::read_line`): bash re-polls per
//! byte, so a peer stalling mid-line past the timeout still blocks here;
//! ready data is read to completion like bash.
//!
//! Unlike bash, a ready `-t 0` reads the line; bash's `-t 0` is a readiness
//! probe that never reads.

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, Trace};

use collect::collect_targets;
use flags::SourceFd;
use flags::parse_flags;
use io::read_line;
use line::LineEnd;
use words::split_fields;

pub(crate) fn run_read(
    line: &[u8],
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "read", cmdline)?;

    let flags = parse_flags(&cmdline.args)?;
    let targets = collect_targets(&cmdline.args)?;

    let prompt_text = flags.prompt.unwrap_or(b"");
    if !prompt_text.is_empty() {
        sys::ERR
            .write_all(prompt_text)
            .change_context(CmdError::Read)?;
    }

    let resolved_fd: Option<sys::LocalFd> = match &flags.source {
        SourceFd::FdVar(var) => {
            let state = cell.borrow().change_context(CmdError::Read)?;
            Some(
                state
                    .fds
                    .get(var)
                    .ok_or(ReadError::VarNotFound)
                    .change_context(CmdError::Read)?
                    .fd
                    .try_clone()
                    .change_context(CmdError::Read)?,
            )
        }
        _ => None,
    };

    let (data, end) = read_line(&flags.source, resolved_fd.as_ref(), &flags)?;
    let failed = matches!(end, LineEnd::Eof | LineEnd::Timeout);
    if data.is_empty() && failed {
        let mut state = cell.borrow_mut().change_context(CmdError::Read)?;
        state.set_last_exit(1);
        return Ok(true);
    }

    let fields = split_fields(&data, targets.len());
    let origin = flags.source.origin();

    let mut state = cell.borrow_mut().change_context(CmdError::Read)?;
    for (i, name) in targets.iter().enumerate() {
        let field = fields.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
        let var_name = name.strip_prefix(b"$").unwrap_or_else(|| name.clone());
        let s = ShortCStr::from_vec(field.to_vec()).change_context(CmdError::Read)?;
        state.set_var(
            var_name,
            ImportedStr::new(s, Trace::at(text.start, origin.clone())),
        );
    }
    state.set_last_exit(if failed { 1 } else { 0 });
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;

mod collect;
mod flags;
mod io;
mod line;
mod read_from_fd;
mod words;
