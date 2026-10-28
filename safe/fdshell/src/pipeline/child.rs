use crate::child;
use crate::error::child_process::ChildProcessError;
use crate::parse::ExpandedStage;
use crate::redirect::Redirect;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{LocalFd, Pid};

use crate::state::ShellState;

mod block;
mod close_inherited;

pub fn run_child(
    i: usize,
    pipes: &[(LocalFd, LocalFd)],
    capture_pairs: &mut [Option<(LocalFd, LocalFd)>],
    children: &[(Pid, LocalFd)],
    stages: &[ExpandedStage<'_>],
    envs: &[crate::run_env::EnvAssigns],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    let stage = stages.get(i).ok_or(ChildProcessError::ExecFailed)?;
    let env = envs.get(i).cloned().ok_or(ChildProcessError::ExecFailed)?;

    let mut redirects: Vec<Redirect> = Vec::new();

    // Clone the pipe ends this stage needs for its stdin/stdout redirects.
    // Stage 0 keeps the shell's stdin: `j + 1 == i` selects the pipe
    // immediately before stage i and can only hold for i >= 1.
    // The clones use try_clone() (lowest free fd), which relies on the
    // inherited pipe ends still occupying the low fd numbers, so the
    // close_inherited cleanup runs after this loop.
    for (j, (read_end, write_end)) in pipes.iter().enumerate() {
        if j + 1 == i {
            let fd = read_end
                .try_clone()
                .change_context(ChildProcessError::RedirectFailed)?;
            redirects.push(Redirect::new(0, fd));
        }
        if j == i {
            let fd = write_end
                .try_clone()
                .change_context(ChildProcessError::RedirectFailed)?;
            redirects.push(Redirect::new(1, fd));
        }
    }

    // Builtin and block stages never exec, so the inherited pipe ends, sibling
    // capture pairs and sibling pidfds would stay open for the whole
    // builtin / interpreter run; close them here (external stages shed them at
    // exec).
    close_inherited::close_inherited(pipes, capture_pairs, children, i)?;

    let opened = crate::redirect::open_redirect_files(stage.redirects(), cell)
        .change_context(ChildProcessError::RedirectFailed)?;

    // The pipe clones above are the stage's **seed**: bash applies a stage's
    // redirections on top of the pipeline's own fds, so `echo e 2>&1 | cat`
    // dup's the pipe write end, not the shell's stdout (LESSONS: "POSIX
    // redirect order is observable"). The stage's own defs are the newer
    // entries, so `> f 2>&1` in a stage still lands fd 2 on `f`.
    let file_redirects =
        crate::redirect::resolve_redirects(stage.redirects(), &opened, &redirects, cell)
            .change_context(ChildProcessError::RedirectFailed)?;
    redirects.extend(file_redirects);

    let child_sock = capture_pairs
        .get_mut(i)
        .and_then(|p| p.take().map(|(_, ch)| ch));

    match stage {
        ExpandedStage::Command(cmd) => child::child_main(child_sock, cell, cmd, &redirects, env),
        ExpandedStage::Block(block) => block::run_block(block, &redirects, cell),
    }
}
