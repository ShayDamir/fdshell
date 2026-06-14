#![forbid(unsafe_code)]

use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_and_capture(cmd: &[u8], cell: &ForkCell<ShellState>) -> Result<Vec<u8>, i32> {
    let (r, w) = sys::pipe::pipe2(sys::fcntl::O_CLOEXEC)?;
    match sys::fork_pidfd::fork_pidfd_cell(cell)? {
        (_, None) => {
            // child stdout → pipe; failure means empty output
            let _ = w.export_to(1);
            // Command substitution output already read; exit code irrelevant
            let _ = crate::repl::run_script(cmd, cell);
            std::process::exit(0);
        }
        (_, Some(pidfd)) => {
            drop(w);
            let mut out = Vec::new();
            let mut buf = [0u8; 4096];
            while let Ok(n) = sys::rw::read(&r, &mut buf) {
                if n == 0 {
                    break;
                }
                if let Some(chunk) = buf.get(..n as usize) {
                    out.extend_from_slice(chunk);
                }
            }
            // Reap child; stdout already consumed above
            let _ = sys::wait_pidfd::wait_pidfd(&pidfd);
            while out.last() == Some(&b'\n') {
                out.pop();
            }
            Ok(out)
        }
    }
}
