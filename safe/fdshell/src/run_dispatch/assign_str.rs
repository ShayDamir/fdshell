//! Bare `NAME=value` assignment storage (the `AssignStr`/`AssignStrs` arms).

use crate::error::cmd::CmdError;
use crate::parse::Assign;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, Trace};

/// Expand and store one `NAME=value` word, tracing the value's origin like
/// any shell assignment. The value is expanded with **its own quote mask**
/// (task #217), so a `\<char>` pair written inside quotes keeps its backslash;
/// the mask `substitute_arg` returns is discarded, because the value is
/// stored whole — no field splitting, no pathname expansion.
pub(crate) fn set(
    assign: &Assign,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let (expanded, _) = crate::substitute::substitute_arg(
        &assign.value,
        &assign.mask,
        &assign.spans,
        &mut HashMap::new(),
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let origin = crate::run_origin::assign_origin(&assign.value, text.origin.clone(), cell)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_var(
        assign.name.clone(),
        ImportedStr::new(expanded, Trace::at(text.start, origin)),
    );
    Ok(())
}
