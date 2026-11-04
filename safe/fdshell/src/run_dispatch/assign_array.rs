//! Running a `name=(w1 w2 …)` statement (task #422). Every element word goes
//! through the ordinary per-word pipeline — substitution, `IFS` splitting,
//! globbing — so bash's element semantics come free: `a=($(echo 1 2))` is two
//! elements, `a=("x y")` one, `a=("*")` one, `a=(*.nonexistent)` the literal
//! pattern. Indices are then sequential from 0, in the order written.

use alloc::vec::Vec;

use core::slice;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, Trace};

use crate::error::cmd::CmdError;
use crate::parse::{ArrayAssign, ArrayWord};
use crate::state::ShellState;

/// Run the whole statement, then leave the state a bare assignment leaves it
/// (`$_` cleared, exit status zeroed — [`super::finish_assignment`]).
pub(crate) fn run(
    assigns: &[ArrayAssign],
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    for assign in assigns {
        let elements = expand(assign, text, cell)?;
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        if assign.append {
            state.append_str_array(&assign.name, elements);
        } else {
            state.set_str_array(&assign.name, elements);
        }
    }
    super::finish_assignment(cell)
}

/// One region's elements, in the order the words were written, each with the
/// origin its own word earned — the same trace bookkeeping a plain assignment
/// keeps ([`crate::run_origin::assign_origin`]).
fn expand(
    assign: &ArrayAssign,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ImportedStr>, Report<CmdError>> {
    let mut out: Vec<ImportedStr> = Vec::new();
    for word in &assign.elements {
        out.extend(expand_word(word, text, cell)?);
    }
    Ok(out)
}

/// One element word through the argument pipeline: the word's own mask, spans
/// and quoted flag, one element per field the pipeline produces (`a=(x y)` is
/// two elements, `a=("x y")` one).
fn expand_word(
    word: &ArrayWord,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ImportedStr>, Report<CmdError>> {
    let fields = crate::substitute::substitute_args(
        slice::from_ref(&word.text),
        slice::from_ref(&word.mask),
        slice::from_ref(&word.spans),
        slice::from_ref(&word.quoted),
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let origin = crate::run_origin::assign_origin(&word.text, text.origin.clone(), cell)?;
    Ok(fields
        .into_iter()
        .map(|field| ImportedStr::new(field, Trace::at(text.start, origin.clone())))
        .collect())
}
