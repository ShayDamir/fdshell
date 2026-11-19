// mutants: -C --lib -C --test -C redirect -C --test -C heredoc -C --test -C env_assign -- -E 'test(/^run::cmd::/) | binary(redirect) | binary(heredoc) | binary(env_assign)'
//! The omitted-name concern of the simple-command path (POSIX #2.9/#2.9.1):
//! the gate that recognizes a command whose word list is empty, and the arm
//! that applies its redirections without launching anything.

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::redirect::Scope;
use crate::run::failing;
use crate::run_env::EnvSave;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// The gate of the omitted-name arm: an empty command word with no argument
/// word and no background suffix, reached either by the parser-built `>f` word 0
/// (whose `command_quoted` is false) or by a word whose expansion produced **no
/// field** ([`ShellState::word0_vanished`], carried in the state because
/// `expand`'s `Cow<CommandLine>` return cannot carry it). Both arms run in the
/// current shell, and neither requires a redirection: `$(true)` alone is in the
/// family. A **quoted** empty word 0 (`"" >f`) is a word — a lookup (bash 127).
pub(super) fn omits_the_name(
    cmdline: &CommandLine,
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
fn is_async(cmdline: &CommandLine) -> bool {
    cmdline.bg_force || cmdline.pidvar.is_some()
}

/// POSIX #2.9 and #2.9.1: a command whose word list is empty — the parse-built
/// `>f` word 0, or a word that **expanded** to no field (`$(true) >f`) — has its
/// command name **omitted**: the redirections apply, nothing launches, `$_` takes
/// the empty command name, and the status is the last command substitution's, 0
/// when none ran. A **quoted** empty word 0 (`"" >f`) is a word, so it stays a
/// lookup (bash 127, fdshell's rc 1) — the gate's `command_quoted` clause is what
/// keeps it out. The prefix **persists in the shell's own store**: there is no
/// command for it to scope to, so bash keeps `VAR=x` from `VAR=x >f` — but with
/// the name omitted it adjusts *no* execution environment (POSIX 2.9.1), so the
/// exports the apply added are put back afterwards; a name already visible to a
/// child keeps its new value.
pub(super) fn run_name_omitted(
    cmdline: &CommandLine,
    save: Option<EnvSave>,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    // A failing open lands here as the statement's own failure (task
    // #179): print, status 1, and the prefix persists exactly as the
    // arm's own rule says it would have.
    let scope = match Scope::open(&cmdline.redirects, cell) {
        Ok(scope) => scope,
        Err(report) => {
            failing::convert(report, cell)?;
            crate::run_env::drop_new_exports(save, cell)?;
            return Ok(());
        }
    };
    scope.run(|| {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        let code = state
            .last_substitution
            .as_ref()
            .map_or(0, |s| s.exit_code());
        state.clear_last_arg();
        state.set_last_exit(code);
        Ok(())
    })?;
    // After the scope: a redirect target is expanded inside it, and that
    // expansion still sees the full apply (`VAR=x >$(printenv VAR)`).
    crate::run_env::drop_new_exports(save, cell)?;
    Ok(())
}
