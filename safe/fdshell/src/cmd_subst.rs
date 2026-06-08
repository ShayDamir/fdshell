#![forbid(unsafe_code)]

use crate::state::ShellState;

pub(crate) fn run_and_capture(cmd: &[u8], state: &ShellState) -> Result<Vec<u8>, i32> {
    let (r, w) = sys::pipe::pipe2(sys::fcntl::O_CLOEXEC)?;
    match sys::fork_pidfd::fork_pidfd()? {
        (_, None) => {
            // SAFETY: child stdout → pipe; failure means empty output
            let _ = w.export_to(1);
            let mut child_state = ShellState::new();
            if let Some(cwd) = state.fds.get(&c"CWD".into())
                && let Ok(c) = cwd.try_clone()
            {
                child_state.fds.insert(c"CWD".into(), c);
            }
            // Command substitution output already read; exit code irrelevant
            let _ = crate::repl::run_script(cmd, &mut child_state);
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
