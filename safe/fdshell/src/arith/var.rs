//! Variable resolution and write-back for arithmetic evaluation: unset/empty
//! values are 0, non-empty values are re-evaluated as expressions (bash).

use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

/// A variable-reference chain longer than this is treated as circular.
const MAX_DEPTH: u32 = 100;

/// Unset or empty variables are 0; a non-empty value is re-evaluated as an
/// arithmetic expression (bash semantics), with a circularity cap.
pub(super) fn eval_var(
    name: &ShortCStr,
    cell: &ForkCell<ShellState>,
    depth: u32,
) -> Result<i64, Report<ResolveError>> {
    // The borrow must not live across the recursive re-evaluation: the value
    // may itself reference shell state.
    let value = {
        let state = crate::substitute::borrow_state(cell)?;
        crate::substitute::resolve::var_value(name, &state).cloned()
    };
    let Some(value) = value else {
        return Ok(0);
    };
    if value.is_empty() {
        return Ok(0);
    }
    if depth + 1 > MAX_DEPTH {
        bail!(ResolveError::ArithCircular { var: name.clone() });
    }
    let bytes = value.as_bytes().change_context(ResolveError::Never)?;
    match super::eval_body(bytes, cell, depth + 1) {
        Ok(v) => Ok(v),
        Err(report) => {
            // A value that is not a parseable expression is "not an integer";
            // deeper evaluation errors (div-by-zero, …) keep their identity.
            if matches!(report.current_context(), ResolveError::ArithSyntax) {
                bail!(ResolveError::ArithNotInteger { var: name.clone() });
            }
            Err(report)
        }
    }
}

pub(super) fn set_var(
    name: &ShortCStr,
    value: i64,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<ResolveError>> {
    let mut state = cell
        .borrow_mut()
        .change_context(ResolveError::RefNotFound)?;
    let text = super::render(value)?;
    let imported = sys::ImportedStr::new(text, sys::Trace::boundary(sys::Origin::Shell));
    state.set_var(name.clone(), imported);
    Ok(())
}
