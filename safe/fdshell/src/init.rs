use error_stack::{Report, ResultExt};
use sys::LocalFd;

use crate::error::shell::ShellInitError;

pub enum FdShellMode {
    Nested(LocalFd),
    Standalone,
}

fn detect_nested() -> Option<sys::ImportedFd> {
    let cookie = std::env::var("FDSHELL_PID")
        .inspect_err(|e| {
            if !matches!(e, std::env::VarError::NotPresent) {
                eprintln!("fdshell: ignoring FDSHELL_PID: {e}");
            }
        })
        .ok()?;
    let pid = match cookie.parse::<u32>() {
        Ok(pid) => pid,
        Err(e) => {
            eprintln!("fdshell: FDSHELL_PID has invalid value: {} ({})", cookie, e);
            return None;
        }
    };
    if pid != std::process::id() {
        return None;
    }
    let sock = std::env::var("FDSHELL_SOCKET")
        .inspect_err(|e| {
            if !matches!(e, std::env::VarError::NotPresent) {
                eprintln!("fdshell: ignoring FDSHELL_SOCKET: {e}");
            }
        })
        .ok()?;
    sys::ImportedFd::from_bytes(sock.as_bytes()).ok()
}

pub fn init_shellfd() -> Result<FdShellMode, Report<ShellInitError>> {
    if let Some(dupfd) = detect_nested() {
        let fd = dupfd
            .try_into_local()
            .change_context(ShellInitError::NestedFd)?;
        sys::shellfd::set_capture_active(true);
        Ok(FdShellMode::Nested(fd))
    } else {
        sys::shellfd::set_capture_active(false);
        Ok(FdShellMode::Standalone)
    }
}
