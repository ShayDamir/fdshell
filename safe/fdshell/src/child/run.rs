use crate::child::Command;
use crate::child::setup_shellfd::setup_shellfd;
use crate::error::child_process::ChildProcessError;
use crate::parse::CommandLine;
use crate::redirect::Redirect;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

mod arm;

/// Run the forked child: the child's own setup, then the dispatch.
///
/// `stage_text` is the gate of the two arms a stage gains over the plain launch
/// (tasks #481 and #381): it is the stage's own text slice, which only the
/// *pipeline* caller has, and only that caller may run a function or an
/// intercepted builtin — a stage is a full command, so its name resolves the way
/// a plain command's does. The plain launch passes `None`: its functions and
/// intercepts belong to `run/parent.rs`, and fdshell's own forked keywords
/// (`timeout 1 f`) must keep resolving as external commands.
pub fn child_main(
    child_sock: Option<sys::LocalFd>,
    cell: &ForkCell<ShellState>,
    cmdline: &CommandLine,
    redirects: &[Redirect],
    env: crate::run_env::EnvAssigns,
    stage_text: Option<&ScriptText>,
) -> Result<i32, Report<ChildProcessError>> {
    let cmd = Command::from(cmdline);
    let args = &cmdline.args;
    setup_shellfd(child_sock.as_ref(), cell)?;
    apply_redirects(redirects)?;

    // A redirections-only stage (the empty command word of task #303): the
    // redirections just applied **are** the command — `>f | cat` creates `f`
    // and the stage exits 0 with `cat` reading an empty stdin. No arg
    // substitution, no `$_` send (bash leaves `$_` alone: there is no command
    // word) and no exec. A quoted empty word 0 (`"" | cat`, bash 127) stays on
    // the normal path, as does an expanded-empty word 0.
    if cmd.name.is_empty() && !cmdline.command_quoted && args.is_empty() {
        return Ok(0);
    }
    let resolved = substitute_args(
        args,
        &cmdline.args_mask,
        &cmdline.args_span,
        &cmdline.args_quoted,
        cell,
    )
    .change_context(ChildProcessError::SubstituteFailed)?;
    if let Some(sock) = &child_sock {
        let last = resolved.last().cloned().unwrap_or_else(|| cmd.name.clone());
        crate::last_arg::send(sock, &last).change_context(ChildProcessError::LastArgSend)?;
    }
    // The scoped prefix applies after substitution: the command's own words
    // were split with the parent's IFS; only its execution environment sees
    // a scoped `IFS` (`IFS=: echo a:b` → `a:b`).
    crate::run_env::apply_child(&env, cell).change_context(ChildProcessError::BorrowFailed)?;
    arm::run(&cmd, &resolved, cmdline, cell, stage_text)
}

/// Apply the child's opened redirects: the fd 0/1 pipe ends of a pipeline
/// stage and every file redirect. The pipeline's block-stage child re-enters
/// the interpreter with them (`pipeline/child/block.rs`), so the rule is shared
/// with the command child.
pub(crate) fn apply_redirects(redirects: &[Redirect]) -> Result<(), Report<ChildProcessError>> {
    for r in redirects {
        r.export()
            .change_context(ChildProcessError::RedirectFailed)?;
    }
    Ok(())
}
