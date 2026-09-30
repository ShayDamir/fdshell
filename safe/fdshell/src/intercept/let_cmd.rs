//! `let expr [expr …]` — evaluate each argument as an arithmetic expression
//! and set the exit status from the last one (bash `let` semantics).
//!
//! Arguments are substituted without IFS splitting or globbing (like bash's
//! `let`), which is why `substitute_arg` is used per argument rather than
//! `substitute_args`.

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, bail};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(crate) fn run_let(
    line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept_no_builtin(line, "let", cmdline)?;
    // `CommandLine.args` excludes the command word, so every entry is an
    // expression; `args` and `args_mask` are parallel at the same offset.
    if cmdline.args.is_empty() {
        bail!(CmdError::LetExpressionExpected);
    }
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let mut status = 0;
    for (i, arg) in cmdline.args.iter().enumerate() {
        let mask = cmdline.args_mask.get(i).cloned().unwrap_or_default();
        let (expanded, _) = crate::substitute::substitute_arg(arg, &mask, &mut cache, cell)
            .change_context(CmdError::Resolve)?;
        let bytes = expanded.as_bytes().change_context(CmdError::Resolve)?;
        let value = crate::arith::eval(bytes, cell).change_context(CmdError::Resolve)?;
        status = i32::from(value == 0);
    }
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(status);
    Ok(true)
}
