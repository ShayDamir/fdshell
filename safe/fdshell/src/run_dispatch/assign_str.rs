//! Bare `NAME=value` assignment storage (the `AssignStr`/`AssignStrs` arms).

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, ShortCStr, Trace};

/// Expand and store one `NAME=value` word, tracing the value's origin like
/// any shell assignment.
pub(crate) fn set(
    var: &ShortCStr,
    value: &ShortCStr,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let (expanded, _) = crate::substitute::substitute_arg(value, &[], &mut HashMap::new(), cell)
        .change_context(CmdError::Resolve)?;
    let origin = crate::run_origin::assign_origin(value, text.origin.clone(), cell)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_var(
        var.clone(),
        ImportedStr::new(expanded, Trace::at(text.start, origin)),
    );
    Ok(())
}
