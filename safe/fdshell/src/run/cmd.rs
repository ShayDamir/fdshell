//! The simple-command path: expand word 0, run the parent-side handlers, then
//! launch the forked child — all of it inside the scoped `NAME=value` prefix.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
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
    // POSIX #2.9: a redirections-only command (an empty command word, the
    // shape `parse::command::redirect_first` builds) runs in the **current**
    // shell: the scope opens and closes around nothing, the status is 0, and a
    // `NAME=value` prefix **persists** — there is no command for it to scope
    // to, so bash keeps `VAR=x` from `VAR=x >f` and the scoped `save` is
    // deliberately not restored here. `command_quoted == false` is the guard
    // that keeps a **quoted** empty word 0 (`"" >f`, bash 127) and an
    // expanded-empty word 0 (`command_word::with_fields` sets the flag) out.
    if is_redirect_only(cmdline) {
        crate::redirect::Scope::open(&cmdline.redirects, cell)?.run(|| {
            cell.borrow_mut()
                .change_context(CmdError::Never)?
                .set_last_exit(0);
            Ok(())
        })?;
        return Ok(None);
    }
    // Word 0 is expanded with the prefix applied, so the command name sees the
    // scoped values (`E=echo; $(echo $E) hi` runs `echo`), as bash expands it.
    // The prefix is restored before the launch: the child applies it itself,
    // after substituting its args (`IFS=: echo a:b` splits in the parent).
    let (cmdline, control) = with_env(save, cell, || {
        let cmdline = crate::command_word::expand(cmdline, cell)?;
        let control = super::parent::run_parent(text, &cmdline, cell)?;
        Ok((cmdline, control))
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

/// The redirections-only command: an empty command word (a **quoted** empty
/// one is a command lookup — `"" >f` is bash 127), no argument word (one makes
/// the statement `>f g`, a command with a redirection), and at least one
/// redirection.
fn is_redirect_only(cmdline: &crate::parse::CommandLine) -> bool {
    cmdline.command.is_empty()
        && !cmdline.command_quoted
        && cmdline.args.is_empty()
        && !cmdline.redirects.is_empty()
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
