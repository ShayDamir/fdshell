use crate::child::setup_shellfd::setup_shellfd;
use crate::child::{self, Command, external};
use crate::error::child_process::ChildProcessError;
use crate::parse::{BuiltinPrefix, CommandLine};
use crate::redirect::Redirect;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// Run the forked child: the child's own setup, then the dispatch.
///
/// `stage_text` is the gate of the user-function arm (task #481): it is the
/// stage's own text slice, which only the *pipeline* caller has, and only that
/// caller may run a function — a stage is a full command, so its name resolves
/// the way a plain command's does. The plain launch passes `None`: its functions
/// belong to `run/parent.rs`, and fdshell's own forked keywords (`timeout 1 f`)
/// must keep resolving as external commands.
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
    let sealed: Vec<sys::ExportedCStr> = resolved.iter().map(|cs| cs.export()).collect();
    let refs: Vec<&CStr> = sealed.iter().map(|rc| rc.as_ref()).collect();

    let state = cell
        .borrow()
        .change_context(ChildProcessError::BorrowFailed)?;

    crate::xtrace::trace(cmd.name.as_bytes().unwrap_or(&[]), &resolved, &state);
    // `run_external` re-borrows the cell to glob the command name, so the
    // trace borrow must end first (RefCell, LESSONS.md). `command NAME`
    // dispatches as a builtin only when the name is in the dispatch table;
    // otherwise it falls through to the external (PATH) search (POSIX #6.6).
    let builtin = match cmd.prefix {
        BuiltinPrefix::Builtin => true,
        BuiltinPrefix::Command => child::dispatch::is_dispatched(&cmd.name),
        BuiltinPrefix::None => child::dispatch::builtin_first(&cmd.name, &state),
    };
    drop(state);

    if builtin {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::BorrowFailed)?;
        child::dispatch::run_builtin(cmd.name.clone(), &refs, args, &state)
    } else if let Some(text) = stage_text
        && let Some(body) = crate::function_call::look_up(cmdline, cell)
            .change_context(ChildProcessError::BorrowFailed)?
    {
        // A stage is a full command, so its name resolves through the same
        // order a plain command uses: user function first, then the builtin
        // table, then `PATH`. `function_call::look_up` is the single owner of
        // the rule (it answers `None` when a `builtin`/`command` prefix is set,
        // so those rows keep reporting). The arm sits at this decision point so
        // it runs after everything `child_main` owns: the capture socket, the
        // stage's redirects, arg substitution, the scoped prefix and the trace.
        child::user_function::run(text, cmdline, &body, cell)
    } else {
        external::run_external(&cmd, &refs, cell)
    }
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
