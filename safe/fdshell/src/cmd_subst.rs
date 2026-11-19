// mutants: -C --lib -C --test -C cmd_subst -C --test -C redirect -C --test -C backtick -C --test -C trap -- -E 'test(/^cmd_subst::/) | binary(cmd_subst) | binary(redirect) | binary(backtick) | binary(trap)'
use crate::error::cmd_subst::CmdSubstError;
use crate::fd_read::{Decision, read_to_end};
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::LocalFd;
use sys::fork_cell::ForkCell;

/// Default maximum number of bytes a command substitution may capture;
/// configurable per shell with `set --stdout-capture-limit <bytes>`.
///
/// Without a cap, an unbounded producer such as `$(yes)` grows the buffer
/// until the process runs out of memory. Exceeding the cap aborts the
/// substitution and kills the child.
pub(crate) const MAX_CAPTURED: usize = 64 * 1024 * 1024;

pub(crate) fn run_and_capture(
    cmd: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<u8>, Report<CmdSubstError>> {
    crate::nest::deeper(cell, CmdSubstError::NestingTooDeep, || {
        // The cap is read before the fork so the child's (forked) state can
        // never change the parent's limit mid-capture.
        let limit = cell
            .borrow()
            .change_context(CmdSubstError::Never)?
            .capture_limit;
        let (r, w) = sys::pipe::pipe2(0).change_context(CmdSubstError::Pipe)?;
        match cell.fork_pidfd().change_context(CmdSubstError::Fork)? {
            (_, None) => {
                // child stdout → pipe; failure means empty output
                let _ = w.export_to(1);
                // Inner scripts run standalone: state changes die with the child,
                // so provenance here is unobservable — position/origin are nominal.
                let text = sys::ScriptText::new(
                    sys::ShortCStr::from_vec(cmd.to_vec()).unwrap_or_default(),
                    sys::Position::new(1, 1),
                    sys::Origin::Shell,
                );
                // Command substitution output already read. The exit code is
                // the script's status: the parent's reap records it as the
                // shell's status the moment the substitution completes (task
                // #533), which is also how a `$(false)` in word-0 position — a
                // command with no name (task #426) — reports 1.
                let _ = crate::repl::run_script(&text, cell);
                let code = cell.borrow().map_or(0, |s| s.last_status.exit_code());
                sys::exit(code)
            }
            (_, Some(pidfd)) => {
                drop(w);
                let out = drain(&r, limit);
                if out.is_err() {
                    // Abandon the pipe so an orphaned producer (e.g. `yes`)
                    // gets SIGPIPE on its next write, and kill the child so
                    // it can't keep running while the parent is idle. Both
                    // are best-effort.
                    drop(r);
                    let _ = pidfd.send_signal(sys::signal::SIGKILL);
                }
                // Reap child; stdout already consumed (or abandoned) above. The
                // status is the shell's from this moment on — bash updates `$?`
                // the moment a substitution completes — and the scratch slot
                // keeps it for the statement the substitution ran for, the only
                // place that can still answer "none ran" (tasks #426, #533).
                if let Ok((status, times)) = pidfd.wait_pidfd_rusage()
                    && let Ok(mut s) = cell.borrow_mut()
                {
                    let code = status.exit_code();
                    s.add_child_times(times);
                    s.last_substitution = Some(status);
                    s.set_last_exit(code);
                }
                out
            }
        }
    })
}

/// Read the pipe until EOF, stripping trailing newlines (command substitution
/// semantics). Fails with [`CmdSubstError::OutputTooLarge`] once `limit` bytes
/// would be captured; the caller must kill the child in that case.
fn drain(r: &LocalFd, limit: usize) -> Result<Vec<u8>, Report<CmdSubstError>> {
    let mut out = Vec::new();
    read_to_end(
        &mut |buf| r.read(buf),
        &mut |chunk| {
            if out.len() + chunk.len() > limit {
                return Decision::Report(Report::new(CmdSubstError::OutputTooLarge));
            }
            out.extend_from_slice(chunk);
            Decision::Resume
        },
        // A caught signal's handler has run and the pipe is still open, so the
        // capture carries on with the bytes it has (bash's rule, task #581 — this
        // is bug #587).
        &|err| {
            if err.errno() == sys::errno::EINTR {
                Decision::Resume
            } else {
                // Preserve prior behavior: any other read error ends the capture.
                Decision::Done
            }
        },
        // The slice a read cannot produce: the old `if let` skipped the chunk.
        &|| Decision::Resume,
    )?;
    while out.last() == Some(&b'\n') {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
