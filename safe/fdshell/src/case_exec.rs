use crate::glob::match_component;
use crate::loop_control::LoopControl;
use crate::substitute::substitute_arg;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;

use crate::error::cmd::CmdError;
use crate::parse::case_block::CaseBlock;
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
    // patterns' side effects unrun (measured).
    crate::redirect::Scope::open(&caseblock.redirects, cell)?.run(|| run_case_body(caseblock, cell))
}

/// The `case` proper: the word, then the first matching clause's body, in the
/// caller's shell.
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

    // POSIX 2.9.4.3 expands a pattern word (tilde, parameter and command
    // substitution, quote removal) **with the pattern's own mask**, and bash
    // expands it lazily: pattern by pattern, stopping at the first match, so
    // `case a in a|$(touch f))` never runs the substitution. `substitute_arg`
    // never splits and never globs, and the mask it returns is bash's
    // pattern-quoting signal — an expansion's bytes inherit the trigger's bit,
    // so `"$p"` keeps an expanded `?` literal and `$p` lets it glob.
    for clause in &caseblock.clauses {
        for pattern in &clause.patterns {
            let (pat, pat_mask) = substitute_arg(
                &pattern.text,
                &pattern.mask,
                &pattern.spans,
                &mut cache,
                cell,
            )
            .change_context(CmdError::Resolve)?;
            let pattern_bytes = pat.as_bytes().change_context(CmdError::Never)?;
            // A word pattern has no FNM_PERIOD rule (LESSONS): dotglob = true.
            if match_component(pattern_bytes, &pat_mask, word_bytes, true) {
                return crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
                    crate::repl::run_script(&clause.body, cell)
                });
            }
        }
    }

    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(None)
}
