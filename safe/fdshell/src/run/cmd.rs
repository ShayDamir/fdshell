//! The simple-command path: expand word 0, run the parent-side handlers, then
//! launch the forked child — all of it inside the scoped `NAME=value` prefix.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::redirect::Scope;
use crate::run_env::EnvSave;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Run one simple command. `Ok(None)`: the command ran in the forked child;
/// `Ok(Some(control))`: a parent-side handler (a function call, an in-process
/// builtin) produced loop control.
pub(crate) fn run_cmd(
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // A scoped `NAME=value` prefix: expanded once, applied around the
    // parent-side handlers, and handed to the forked child (which applies it
    // itself after arg substitution).
    let env = crate::run_env::expand(cmdline, text, cell)?;
    let save = crate::run_env::apply(&env, cell)?;
    // Word 0 is expanded with the prefix applied, so the command name sees the
    // scoped values (`E=echo; $(echo $E) hi` runs `echo`), as bash expands it.
    // The prefix is restored before the launch: the child applies it itself,
    // after substituting its args (`IFS=: echo a:b` splits in the parent).
    let cmdline = crate::command_word::expand(cmdline, cell)?;
    // POSIX #2.9 and #2.9.1: a command whose word list is empty — the parse-built
    // `>f` word 0, or a word that **expanded** to no field (`$(true) >f`) — has
    // its command name **omitted**: the redirections apply, nothing launches,
    // `$_` takes the empty command name, and the status is the last command
    // substitution's, 0 when none ran. A **quoted** empty word 0 (`"" >f`) is a
    // word, so it stays a lookup (bash 127, fdshell's rc 1) — the gate's
    // `command_quoted` clause is what keeps it out. The prefix **persists**:
    // there is no command for it to scope to, so bash keeps `VAR=x` from
    // `VAR=x >f` and the scoped `save` is deliberately not restored here.
    if omits_the_name(&cmdline, cell)? {
        Scope::open(&cmdline.redirects, cell)?.run(|| {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            let code = state
                .last_substitution
                .as_ref()
                .map_or(0, |s| s.exit_code());
            state.clear_last_arg();
            state.set_last_exit(code);
            Ok(())
        })?;
        return Ok(None);
    }
    let control = with_env(save, cell, || {
        super::parent::run_parent(text, &cmdline, cell)
    })?;
    if let Some(control) = control {
        return Ok(control);
    }
    // Forked commands must expand their args against the previous `$_`,
    // so the child (not the parent) reports the new value via the capture
    // socket, consumed by `finish_cmd`.
    let outcome = crate::launch::launch(cell, &cmdline, &env).change_context(CmdError::Launch)?;
    {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.last_status =
            crate::postlaunch::finish_cmd(cmdline.into_owned(), outcome, &mut state)
                .change_context(CmdError::Launch)?;
    }
    Ok(None)
}

/// The gate of the omitted-name arm: an empty command word with no argument
/// word and no background suffix, reached either by the parser-built `>f` word 0
/// (whose `command_quoted` is false) or by a word whose expansion produced **no
/// field** ([`ShellState::word0_vanished`], carried in the state because
/// `expand`'s `Cow<CommandLine>` return cannot carry it). Both arms run in the
/// current shell, and neither requires a redirection: `$(true)` alone is in the
/// family. A **quoted** empty word 0 (`"" >f`) is a word — a lookup (bash 127).
fn omits_the_name(
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    if !cmdline.command.is_empty() || !cmdline.args.is_empty() || is_async(cmdline) {
        return Ok(false);
    }
    let state = cell.borrow().change_context(CmdError::Never)?;
    Ok(state.word0_vanished || !cmdline.command_quoted)
}

/// A background statement keeps its fork — `&` is a job, so the arm runs inside
/// the forked job — and the pid-var form `&>&x` is registered under its name.
/// Both are pinned as a family (`tests/redirect.rs`, `tests/heredoc.rs`).
fn is_async(cmdline: &crate::parse::CommandLine) -> bool {
    cmdline.bg_force || cmdline.pidvar.is_some()
}

/// Run `f` with the scoped prefix applied, restoring it on both paths (the
/// restore is best-effort on the error path, as in `run_parent`).
fn with_env<T>(
    save: Option<EnvSave>,
    cell: &ForkCell<ShellState>,
    f: impl FnOnce() -> Result<T, Report<CmdError>>,
) -> Result<T, Report<CmdError>> {
    let result = f();
    if result.is_err() {
        let _ = crate::run_env::restore(save, cell);
        return result;
    }
    crate::run_env::restore(save, cell)?;
    result
}
