//! Run the arithmetic `for ((init;cond;step)) ; do … ; done` (bash's
//! `FOR_ACTION`): init once, then condition, body, step — the shape XCU 2.9.3
//! states and bash implements. Each clause is evaluated **in-process** by
//! [`crate::arith::eval`], exactly as the `((expr))` command evaluates its
//! body, so an assignment made by a clause survives the loop.

use crate::error::cmd::CmdError;
use crate::loop_control::{AtLevel, LoopControl, at_level, in_loop};
use crate::parse::for_block::arith::ArithForBlock;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

pub(crate) fn run(
    block: &ArithForBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    in_loop(cell, || run_loop(block, cell))
}

fn run_loop(
    block: &ArithForBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    eval(&block.init, cell)?;
    let mut ran_body = false;
    loop {
        // A clause written empty is a no-op; an empty condition is true, so
        // `for ((;;))` runs until its body breaks out of it.
        if eval(&block.cond, cell)?.is_some_and(|value| value == 0) {
            break;
        }
        ran_body = true;
        match run_body(block, cell)? {
            // `break` skips the step, `break 2`/`return`/errexit leave the loop
            // whole, and `continue` runs the step before the condition is
            // re-tested (bash's `FOR_ACTION` control flow).
            Some(control) => match at_level(control) {
                AtLevel::Stop => break,
                AtLevel::Next => step(block, cell)?,
                AtLevel::Outer(rest) => return Ok(Some(rest)),
            },
            None => step(block, cell)?,
        }
    }
    // The status of a loop whose body never ran is 0, not the last condition's
    // (the rule of task #164, shared with `while`/`until` and the word `for`).
    if !ran_body {
        cell.borrow_mut()
            .change_context(CmdError::Never)?
            .set_last_exit(0);
    }
    Ok(None)
}

/// One body iteration under the nesting cap; `Some` on break/continue/return.
fn run_body(
    block: &ArithForBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
        crate::repl::run_script(&block.body, cell)
    })
}

/// The step clause, run after every body that did not `break` out of the loop.
fn step(block: &ArithForBlock, cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    eval(&block.step, cell).map(|_| ())
}

/// Evaluate one clause in-process; `None` in, `None` out for a clause written
/// empty. A clause never touches `last_status`: bash's loop status is the body's
/// last command, and a clause's own value is read only for the condition.
fn eval(
    clause: &Option<ScriptText>,
    cell: &ForkCell<ShellState>,
) -> Result<Option<i64>, Report<CmdError>> {
    let Some(clause) = clause else {
        return Ok(None);
    };
    let bytes = clause.as_bytes().change_context(CmdError::Resolve)?;
    crate::arith::eval(bytes, cell)
        .change_context(CmdError::Resolve)
        .map(Some)
}
