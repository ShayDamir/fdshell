use crate::envfilter::glob_match;
use crate::loop_control::LoopControl;
use error_stack::{Report, ResultExt};
use std::collections::HashMap;

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
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let word = crate::substitute::substitute_arg(&caseblock.word, &mut cache, cell)
        .change_context(CmdError::Resolve)?;
    let word_bytes = word.as_bytes();

    for clause in &caseblock.clauses {
        for pattern in &clause.patterns {
            let pattern_bytes = pattern.as_bytes().change_context(CmdError::Exec)?;
            if glob_match(pattern_bytes, word_bytes) {
                let body = clause.body.as_bytes().change_context(CmdError::Exec)?;
                return crate::repl::run_script(body, cell);
            }
        }
    }

    let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
    state.set_last_exit(0);
    Ok(None)
}
