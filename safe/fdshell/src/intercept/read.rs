// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

//! `read` builtin: reads one line from stdin, `-u fd`, or an fd var.
//!
//! `-t` is a one-shot `poll` gate (see `io::read_line`): bash re-polls per
//! byte, so a peer stalling mid-line past the timeout still blocks here;
//! ready data is read to completion like bash.
//!
//! Unlike bash, a ready `-t 0` reads the line; bash's `-t 0` is a readiness
//! probe that never reads.
//!
//! The `-p` prompt is drawn only when the resolved source fd is a terminal —
//! bash tests the fd it reads from (stdin, `-u <rawfd>`, or `-u %FDVAR`), not
//! stdin, and the fd is resolved before the prompt is written.

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
use flags::source_is_tty;
use io::read_line;
use line::LineEnd;
use putback::{put_back, take_pending};
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

    // bash draws the prompt only when the fd it is about to read from is a
    // terminal, so a resolved-fd failure (a missing var) precedes the write.
    if !prompt_text.is_empty() && source_is_tty(&flags.source, resolved_fd.as_ref())? {
        sys::ERR
            .write_all(prompt_text)
            .change_context(CmdError::Read)?;
    }

    // The source's putback buffer (task #609): what an earlier read of it did not
    // consume. The drain-first reader serves it before any syscall, and whatever
    // the line leaves behind goes back for the next read on the same source.
    let mut pending = take_pending(cell, &flags.source, resolved_fd.as_ref())?;
    let (data, end) = read_line(&flags.source, resolved_fd.as_ref(), &flags, &mut pending)?;
    put_back(cell, &flags.source, resolved_fd.as_ref(), pending)?;

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
mod putback;
mod read_from_fd;
mod words;
