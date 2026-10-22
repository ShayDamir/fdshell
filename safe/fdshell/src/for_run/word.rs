//! One word of the `for` list, and the body under it: extracted from
//! `for_run.rs` for the line budget (task #298 added the closer's
//! redirections there).

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::for_block::ForBlock;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// One word of the for list: for an fd for-var, an array reference in the
/// word list expands each entry as a dup; any other word binds as a string.
pub(super) fn run_for_word(
    forblock: &ForBlock,
    word: &sys::ImportedStr,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let dups = if forblock.var.starts_with(b"%") {
        super::array_word::dup_array_word(word, cell)?
    } else {
        None
    };
    match dups {
        Some(dups) => {
            let name = forblock
                .var
                .strip_prefix(b"%")
                .unwrap_or(forblock.var.clone());
            for fdvar in dups {
                cell.borrow_mut()
                    .change_context(CmdError::Never)?
                    .set_fd_var(name.clone(), fdvar);
                if let Some(control) = run_body(forblock, cell)? {
                    return Ok(Some(control));
                }
            }
            Ok(None)
        }
        None => {
            cell.borrow_mut()
                .change_context(CmdError::Never)?
                .set_var(forblock.var.clone(), word.clone());
            run_body(forblock, cell)
        }
    }
}

/// Run the for body under the nesting cap; `Some` on break/continue/return.
fn run_body(
    forblock: &ForBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
        crate::repl::run_script(&forblock.body, cell)
    })
}
