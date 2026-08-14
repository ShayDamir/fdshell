use crate::error::cmd_subst::CmdSubstError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::LocalFd;
use sys::fork_cell::ForkCell;

pub(crate) fn run_and_capture(
    cmd: &[u8],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<u8>, Report<CmdSubstError>> {
    crate::nest::deeper(cell, CmdSubstError::NestingTooDeep, || {
        let (r, w) = sys::pipe::pipe2(0).change_context(CmdSubstError::Pipe)?;
        match sys::fork_pidfd::fork_pidfd_cell(cell).change_context(CmdSubstError::Fork)? {
            (_, None) => {
                // child stdout → pipe; failure means empty output
                let _ = w.export_to(1);
                // Command substitution output already read; exit code irrelevant
                let _ = crate::repl::run_script(cmd, cell);
                sys::exit(0)
            }
            (_, Some(pidfd)) => {
                drop(w);
                let out = drain(&r);
                // Reap child; stdout already consumed above
                let _ = sys::wait_pidfd::wait_pidfd(&pidfd);
                Ok(out)
            }
        }
    })
}

/// Read the pipe until EOF, stripping trailing newlines (command substitution semantics).
fn drain(r: &LocalFd) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    while let Ok(n) = sys::rw::read(r, &mut buf) {
        if n == 0 {
            break;
        }
        if let Some(chunk) = buf.get(..n) {
            out.extend_from_slice(chunk);
        }
    }
    while out.last() == Some(&b'\n') {
        out.pop();
    }
    out
}
