//! `local NAME[=value] …` — string variables scoped to the current function
//! call. A value is expanded with `substitute_arg` and the value word's own
//! quote mask (no IFS splitting, no globbing — the bare-assignment rule, as
//! `run_dispatch/assign_str.rs`), and the name comes from the raw word. A bare
//! `NAME` declares the variable unset for the call; `local` with no arguments
//! lists the call's locals.
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
use crate::parse::{CommandLine, value_mask, value_spans};
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

/// One word per `local` argument: the name, and the expanded value. The word's
/// mask comes from `args_mask` by index (never `zip`, so a length mismatch
/// cannot silently drop a word); the value's mask and zero-byte span list are
/// that word's mask and spans sliced at the `=`.
fn expand_words(
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<(ShortCStr, Option<ImportedStr>)>, Report<CmdError>> {
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let mut words = Vec::new();
    let (masks, spans_at) = (cmdline.args_mask.as_slice(), cmdline.args_span.as_slice());
    for (i, word) in cmdline.args.iter().enumerate() {
        let (name, raw) = split_name(word)?;
        let mask = value_mask(masks.get(i).map_or(&[], Vec::as_slice), name.len());
        let spans = value_spans(spans_at.get(i).map_or(&[], Vec::as_slice), name.len());
        let value = expand_value(raw, &mask, &spans, text, &mut cache, cell)?;
        words.push((name, value));
    }
    Ok(words)
}

/// The `=value` part, with no splitting and no globbing, traced like any shell
/// assignment (`run_env::expand_one` and `assign_str::set` are the precedents).
/// The value is expanded with its own quote mask, so a quoted escape pair
/// keeps its backslash (task #217).
fn expand_value(
    raw: Option<ShortCStr>,
    mask: &[bool],
    spans: &[usize],
    text: &ScriptText,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<Option<ImportedStr>, Report<CmdError>> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let (expanded, _) = crate::substitute::substitute_arg(&value, mask, spans, cache, cell)
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
    let assigned = word.split_once_byte(b'=').map(|(n, v)| (n, Some(v)));
    let (name, value) = assigned.unwrap_or((word.clone(), None));
    if name.is_empty() || name.starts_with(b"%") || name.starts_with(b"-") {
        bail!(CmdError::LocalBadName { name: name.clone() });
    }
    Ok((name, value))
}

#[cfg(test)]
mod tests;
