//! `local NAME[=value] …` — string variables scoped to the current function
//! call. A value is expanded with `substitute_arg` (no IFS splitting, no
//! globbing — the bare-assignment rule, as `run_dispatch/assign_str.rs`), and
//! the name comes from the raw word. A bare `NAME` declares the variable unset
//! for the call; `local` with no arguments lists the call's locals.
//!
//! The shadowed values are restored when the function call returns
//! (`state/frames.rs`), which is why `local` is legal only inside a function.

mod list;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail, ensure};
use hashbrown::HashMap;
use sys::fork_cell::ForkCell;
use sys::{ExportedFd, ImportedStr, ScriptText, ShortCStr, Trace};

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;

pub(crate) fn run_local(
    line: &[u8],
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept_no_builtin(line, "local", cmdline)?;
    {
        // A shared borrow, scoped so it is dropped before any later borrow.
        let state = cell.borrow().change_context(CmdError::Never)?;
        ensure!(state.in_frame(), CmdError::LocalOutsideFunction);
    }
    if cmdline.args.is_empty() {
        list::list_locals(cell)?;
        return Ok(true);
    }
    let words = expand_words(cmdline, text, cell)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    for (name, value) in words {
        state.shadow(&name)?;
        store(&mut state, &name, value);
    }
    state.set_last_exit(0);
    Ok(true)
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

/// One word per `local` argument: the name, and the expanded value.
fn expand_words(
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<(ShortCStr, Option<ImportedStr>)>, Report<CmdError>> {
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let mut words = Vec::new();
    for word in &cmdline.args {
        let (name, raw) = split_name(word)?;
        words.push((name, expand_value(raw, text, &mut cache, cell)?));
    }
    Ok(words)
}

/// The `=value` part, with no splitting and no globbing, traced like any shell
/// assignment (`run_env::expand_one` and `assign_str::set` are the precedents).
fn expand_value(
    raw: Option<ShortCStr>,
    text: &ScriptText,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<Option<ImportedStr>, Report<CmdError>> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let (expanded, _) = crate::substitute::substitute_arg(&value, &[], cache, cell)
        .change_context(CmdError::Resolve)?;
    let origin = crate::run_origin::assign_origin(&value, text.origin.clone(), cell)?;
    Ok(Some(ImportedStr::new(
        expanded,
        Trace::at(text.start, origin),
    )))
}

/// `NAME=value` splits into (NAME, Some(value)); a bare word is (NAME, None).
/// A name is non-empty, outside the `%` fd-var namespace (fd scoping is task
/// #46), and not an option word, so `-r`/`-a`/`-n` are rejected as names.
fn split_name(word: &ShortCStr) -> Result<(ShortCStr, Option<ShortCStr>), Report<CmdError>> {
    let (name, value) = match word.split_once_byte(b'=') {
        Some((name, value)) => (name, Some(value)),
        None => (word.clone(), None),
    };
    if name.is_empty() || name.starts_with(b"%") || name.starts_with(b"-") {
        bail!(CmdError::LocalBadName { name: name.clone() });
    }
    Ok((name, value))
}

#[cfg(test)]
mod tests;
