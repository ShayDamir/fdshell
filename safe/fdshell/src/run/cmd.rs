// mutants: -C --lib -C --test -C redirect -C --test -C heredoc -C --test -C env_assign -- -E 'test(/^run::cmd::/) | binary(redirect) | binary(heredoc) | binary(env_assign)'
//! The simple-command path: expand word 0, run the parent-side handlers, then
//! launch the forked child — all of it inside the scoped `NAME=value` prefix.
//! The omitted-name arm lives in [`gate`].

mod gate;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::run::failing;
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
    // POSIX #2.9 and #2.9.1: an empty command word is [`gate`]'s concern.
    if gate::omits_the_name(&cmdline, cell)? {
        gate::run_name_omitted(&cmdline, save, cell)?;
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
    let outcome = match crate::launch::launch(cell, &cmdline, &env) {
        // The launch classifier decides inside the error arm (no match guard: a
        // guard the tests can only ever answer one way is a mutant site whose
        // family no pin can kill — CATCHING_MUTANTS rule 30). The classification
        // and both answers stay, pinned by `run/failing/tests.rs` and
        // `tests/redirect.rs`.
        Err(report) => {
            if failing::classifies_launch(&report) {
                failing::fail(&report, cell)?;
                return Ok(None);
            }
            Err(report.change_context(CmdError::Launch))?
        }
        Ok(outcome) => outcome,
    };
    {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.last_status =
            crate::postlaunch::finish_cmd(cmdline.into_owned(), outcome, &mut state)
                .change_context(CmdError::Launch)?;
    }
    Ok(None)
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
