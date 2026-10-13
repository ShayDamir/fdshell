//! `set --`: rebind `$1..` to the expanded words after `--`, leaving `$0`
//! alone (LESSONS.md: "Positional parameters: the deque is `$1..$N`, and `$0`
//! is a separate parameter"). The arm stores its own expanded `$_`, which is
//! why [`super::last_arg_frame`] skips `set --`.

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Trace};

/// `set -- a b c`: replace `$1..` with the expanded words after `--` (an empty
/// deque for a bare `set --`), leave `$0` alone, set `$_` to the last expanded
/// word (the `set` command word when there are none), status 0.
pub(super) fn run_set_positional(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "set", cmdline)?;
    let expanded = crate::substitute::substitute_args(
        cmdline.args.get(1..).unwrap_or(&[]),
        cmdline.args_mask.get(1..).unwrap_or(&[]),
        cmdline.args_quoted.get(1..).unwrap_or(&[]),
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_arg(expanded.last().cloned().unwrap_or_else(|| c"set".into()));
    let positional = expanded
        .iter()
        .map(|s| ImportedStr::new(s.clone(), Trace::at(text.start, text.origin.clone())))
        .collect();
    state.set_positional(positional);
    state.set_last_exit(0);
    Ok(true)
}
