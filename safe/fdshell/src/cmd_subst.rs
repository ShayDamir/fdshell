use crate::error::cmd_subst::CmdSubstError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
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
        match sys::fork_pidfd::fork_pidfd_cell(cell).change_context(CmdSubstError::Fork)? {
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
                // Command substitution output already read; exit code irrelevant
                let _ = crate::repl::run_script(&text, cell);
                sys::exit(0)
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
                    let _ = sys::pidfd_send_signal::send_signal(
                        &pidfd,
                        sys::pidfd_send_signal::SIGKILL,
                    );
                }
                // Reap child; stdout already consumed (or abandoned) above.
                let _ = sys::wait_pidfd::wait_pidfd(&pidfd);
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
    let mut buf = [0u8; 4096];
    loop {
        let n = match sys::rw::read(r, &mut buf) {
            // Preserve prior behavior: a read error ends the capture.
            Err(_) => break,
            Ok(0) => break,
            Ok(n) => n,
        };
        if let Some(chunk) = buf.get(..n) {
            if out.len() + chunk.len() > limit {
                bail!(CmdSubstError::OutputTooLarge);
            }
            out.extend_from_slice(chunk);
        }
    }
    while out.last() == Some(&b'\n') {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
