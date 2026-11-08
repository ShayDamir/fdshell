//! `set --`: rebind `$1..` to the expanded words after `--`, leaving `$0`
//! alone (LESSONS.md: "Positional parameters: the deque is `$1..$N`, and `$0`
//! is a separate parameter"). `$_` is the shared rule over **all** the words
//! the command consumed, the `--` word itself included — a bare `set --`
//! leaves `--` there, as bash does — which is why [`super::last_arg_frame`]
//! skips `set --`: this arm decides which form ran.

// mutants: -C --lib -C --test -C last_arg -C --test -C set_cmd -- -E 'test(/^intercept::set_positional::/) | binary(last_arg) | binary(set_cmd)'

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ShortCStr, Trace};

/// `set -- a b c`: replace `$1..` with the expanded words after `--` (an empty
/// deque for a bare `set --`), leave `$0` alone, set `$_` to the last consumed
/// word (the `--` word itself when no operands ran), status 0.
pub(super) fn run_set_positional(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "set", cmdline)?;
    let consumed = crate::substitute::substitute_args(
        &cmdline.args,
        &cmdline.args_mask,
        &cmdline.args_span,
        &cmdline.args_quoted,
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_arg(crate::last_arg::last_word(
        &consumed,
        &ShortCStr::from(c"set"),
    ));
    let positional = consumed
        .get(1..)
        .unwrap_or(&[])
        .iter()
        .map(|s| ImportedStr::new(s.clone(), Trace::at(text.start, text.origin.clone())))
        .collect();
    state.set_positional(positional);
    state.set_last_exit(0);
    Ok(true)
}
