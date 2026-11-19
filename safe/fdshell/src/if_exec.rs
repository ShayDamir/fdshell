// mutants: -C --lib -C --test -C cond_blocks -C --test -C errexit -C --test -C set_opts -- -E 'test(/^if_exec::/) | test(/^tests::run::/) | binary(cond_blocks) | binary(errexit) | binary(set_opts)'

use crate::loop_control::LoopControl;
use error_stack::{Report, ResultExt};

use crate::cond::{body_bytes, errexit};
use crate::error::cmd::CmdError;
use crate::parse::if_block::IfBlock;
use crate::run::failing;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_if(
    ifblock: &IfBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // The redirections written after `fi` belong to the whole `if` (task #298):
    // the scope opens before the condition runs, so a failing open fails the
    // command, and it restores on every exit path (`Scope::run`). A failing
    // open lands as the `if`'s status 1 and the script continues (task #179).
    let scope = match crate::redirect::Scope::open(&ifblock.redirects, cell) {
        Ok(scope) => scope,
        Err(report) => {
            failing::convert(report, cell)?;
            return Ok(None);
        }
    };
    scope.run(|| run_if_body(ifblock, cell))
}

/// The `if` proper: the condition, the taken arm's body, the `elif` chain, and
/// the `else` body, all in the caller's shell.
fn run_if_body(
    ifblock: &IfBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let cond_bodies = body_bytes(&ifblock.cond_bodies);
    // The condition runs inside the errexit gate (task #491): bash exempts the
    // test *and* everything a compound written in it executes, and only the
    // flag reaches through the nesting.
    let control = errexit::gated(cell, || {
        crate::repl::run_cond_list(
            &ifblock.condition,
            &cond_bodies,
            cell,
            ifblock.cond_background,
        )
    })?;
    // Boundary after the condition (outside the errexit gate); plan's merge rule.
    let control = crate::trap_exec::run_pending(cell)?.or(control);
    if let Some(control) = control {
        // An `if` is not a loop level (README "Loop control"), so the signal is
        // the caller's: propagate it whole, consuming nothing.
        return Ok(Some(control));
    }
    let exit_code = {
        let state = cell.borrow().change_context(CmdError::Never)?;
        state.last_status.exit_code()
    };
    if exit_code == 0 {
        return crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
            crate::repl::run_script(&ifblock.then_body, cell)
        });
    }
    for arm in &ifblock.elifs {
        let arm_bodies = body_bytes(&arm.cond_bodies);
        let control = errexit::gated(cell, || {
            crate::repl::run_cond_list(&arm.cond, &arm_bodies, cell, arm.cond_background)
        })?;
        let control = crate::trap_exec::run_pending(cell)?.or(control);
        if let Some(control) = control {
            return Ok(Some(control));
        }
        let ec_exit = {
            let state = cell.borrow().change_context(CmdError::Never)?;
            state.last_status.exit_code()
        };
        if ec_exit == 0 {
            return crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
                crate::repl::run_script(&arm.body, cell)
            });
        }
    }
    if let Some(ref else_body) = ifblock.else_body {
        return crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
            crate::repl::run_script(else_body, cell)
        });
    } else {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
