//! Bare `NAME=value` assignment storage (the `AssignStr`/`AssignStrs` arms).

use crate::error::cmd::CmdError;
use crate::parse::Assign;
use crate::parse::array_ref;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, Trace};

/// Expand and store one `NAME=value` word, tracing the value's origin like
/// any shell assignment. The value is expanded with **its own quote mask**
/// (task #217), so a `\<char>` pair written inside quotes keeps its backslash;
/// the mask `substitute_arg` returns is discarded, because the value is
/// stored whole — no field splitting, no pathname expansion. The two array
/// routings below are task #422's (`a[1]=x`, and a plain write to a name the
/// array store holds).
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
    let value = ImportedStr::new(expanded, Trace::at(text.start, origin));
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    // bash's bracket rule (task #422): `a[1]=x` is an **element** write, and a
    // plain write to a name the array store already holds writes element 0
    // (`a=(1 2); a=scalar; echo "${a[@]}"` → `scalar 2`). Every other name keeps
    // the scalar path exactly as it was.
    if let Some((base, index)) = array_ref::split_index_ref(&assign.name) {
        state.set_str_element(&base, index, value);
        return Ok(());
    }
    if state.holds_str_array(&assign.name) {
        state.set_str_element(&assign.name, 0, value);
        return Ok(());
    }
    state.set_var(assign.name.clone(), value);
    Ok(())
}
