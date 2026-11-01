//! `local NAME[=value] …` — string variables scoped to the current function
//! call. A value is expanded with `substitute_arg` and the value word's own
//! quote mask (no IFS splitting, no globbing — the bare-assignment rule, as
//! `run_dispatch/assign_str.rs`), and the name comes from the raw word. A bare
//! `NAME` declares the variable unset for the call; `local` with no arguments
//! lists the call's locals.
//!
//! The shadowed values are restored when the function call returns
//! (`state/frames.rs`), which is why `local` is legal only inside a function.

mod expand;
mod list;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, ShortCStr};

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;

pub(crate) fn run_local(
    line: &[u8],
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<Vec<ShortCStr>>, Report<CmdError>> {
    super::validation::validate_intercept_no_builtin(line, "local", cmdline)?;
    {
        // A shared borrow, scoped so it is dropped before any later borrow.
        let state = cell.borrow().change_context(CmdError::Never)?;
        ensure!(state.in_frame(), CmdError::LocalOutsideFunction);
    }
    if cmdline.args.is_empty() {
        list::list_locals(cell)?;
        return Ok(Some(Vec::new()));
    }
    let words = expand::expand_words(cmdline, text, cell)?;
    let consumed = consumed_words(&words);
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    for (name, value) in words {
        state.shadow(&name)?;
        store(&mut state, &name, value);
    }
    state.set_last_exit(0);
    Ok(Some(consumed))
}

/// The words `local` consumed, rebuilt with the **expanded** values, because
/// bash's `$_` is the last argument after expansion (`x=hello` for
/// `local "x=$V"`). A word whose value did not expand rebuilds to the raw word,
/// and a bare name is reported as-is, so the existing pins stay put.
fn consumed_words(words: &[(ShortCStr, Option<ImportedStr>)]) -> Vec<ShortCStr> {
    let eq: ShortCStr = c"=".into();
    words
        .iter()
        .map(|(name, value)| match value {
            Some(value) => ShortCStr::concat(&[name, &eq, &value.value]),
            None => name.clone(),
        })
        .collect()
}

/// Store one shadowed word: `Some` assigns and exports it (so children forked
/// inside the call see it, as bash), `None` — the declare form `local NAME` —
/// leaves the name unset. The export attribute stays global in bash, so the
/// declare form leaves `exports` untouched.
fn store(state: &mut ShellState, name: &ShortCStr, value: Option<ImportedStr>) {
    match value {
        Some(v) => {
            state.set_var(name.clone(), v.clone());
            let _ = state.exports.insert(name.clone(), v);
        }
        None => {
            let _ = state.strings.remove(name);
        }
    }
}

#[cfg(test)]
mod tests;
