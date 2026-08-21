use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Trace};

/// Replace positional parameters with the args after `--`.
/// Only `set --` is intercepted; other `set` forms fall through to external lookup.
pub(crate) fn run_set(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    if !cmdline.args.first().is_some_and(|a| a.eq_bytes(b"--")) {
        return Ok(false);
    }
    super::validation::validate_intercept(line, "set", cmdline)?;
    let expanded = crate::substitute::substitute_args(
        cmdline.args.get(1..).unwrap_or(&[]),
        cmdline.args_fq.get(1..).unwrap_or(&[]),
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let positional = expanded
        .iter()
        .map(|s| ImportedStr::new(s.clone(), Trace::at(text.start, text.origin.clone())))
        .collect();
    state.set_positional(positional);
    state.set_last_exit(0);
    Ok(true)
}
