// mutants: -C --lib -C --test -C case -C --test -C trap -- -E 'test(/^case_exec::/) | binary(case) | binary(trap)'

mod pattern;

use crate::loop_control::LoopControl;
use crate::parse::case_clause::terminator::Terminator;
use crate::substitute::substitute_arg;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;

use crate::error::cmd::CmdError;
use crate::parse::case_block::CaseBlock;
use crate::run::failing;
use crate::state::ShellState;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(crate) fn run_case(
    caseblock: &CaseBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // The redirections written after `esac` belong to the whole `case` (task
    // #298). The scope opens **before** the `case` word is expanded: bash opens
    // the redirection first, so a failing open leaves the word's and the
    // patterns' side effects unrun (measured). A failing open lands as the
    // `case`'s status 1 and the script continues (task #179).
    let scope = match crate::redirect::Scope::open(&caseblock.redirects, cell) {
        Ok(scope) => scope,
        Err(report) => {
            failing::convert(report, cell)?;
            return Ok(None);
        }
    };
    scope.run(|| run_case_body(caseblock, cell))
}

/// The `case` proper: the word, then the clause walk, in the caller's shell.
///
/// bash's walk runs the first clause whose pattern matches, and the terminator
/// of a clause that **ran** says what happens next (`case_clause::terminator`):
/// `;;` stops, `;&` runs the next clause's body, `;;&` tests the next clause's
/// patterns against the same word. A `;&`-forced clause runs **without** testing
/// its patterns, so it never expands them; a `;;&` retest does expand and test
/// (both pinned).
fn run_case_body(
    caseblock: &CaseBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (word, _) = substitute_arg(
        &caseblock.word,
        &caseblock.word_mask,
        &caseblock.word_span,
        &mut cache,
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let word_bytes = word.as_bytes().change_context(CmdError::Never)?;

    // `force` is the `;&` of the clause that ran last: it runs this clause's body
    // without asking its patterns. `ran` is the guard of the status rule below.
    let mut force = false;
    let mut ran = false;
    for clause in &caseblock.clauses {
        if !force && !pattern::matches(clause, word_bytes, &mut cache, cell)? {
            continue;
        }
        force = false;
        ran = true;
        let control = crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
            crate::repl::run_script(&clause.body, cell)
        })?;
        // Boundary after the clause body; the action's control wins (plan's rule).
        let control = crate::trap_exec::run_pending(cell)?.or(control);
        if control.is_some() {
            // `case` is a transparent level (LESSONS "A cond-list run returns loop
            // control"): a `break`/`continue`/`exit` raised in any body of the
            // chain propagates whole, and the status of the last run body stays.
            return Ok(control);
        }
        match clause.terminator {
            Terminator::Stop => break,
            Terminator::RunNext => force = true,
            Terminator::Retest => {}
        }
    }
    // POSIX: the status is zero only when nothing ran; otherwise the last run
    // body owns it (pinned: `a) false;;& b) nope;;` is rc 1 in bash).
    if !ran {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
    }
    Ok(None)
}
