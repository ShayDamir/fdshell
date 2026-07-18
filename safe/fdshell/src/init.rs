use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::LocalFd;

use crate::error::shell::ShellInitError;

pub enum FdShellMode {
    Nested(LocalFd),
    Standalone,
}

fn detect_nested() -> Option<sys::ImportedFd> {
    let cookie_val = sys::env::getenv(c"FDSHELL_PID")?;
    let cookie_str_bytes = cookie_val.as_bytes().ok()?;
    let cookie_str = match core::str::from_utf8(cookie_str_bytes) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(
                crate::io::Stderr,
                "fdshell: FDSHELL_PID has invalid UTF-8: {e}"
            );
            return None;
        }
    };
    let pid = match cookie_str.parse::<u32>() {
        Ok(pid) => pid,
        Err(e) => {
            let _ = writeln!(
                crate::io::Stderr,
                "fdshell: FDSHELL_PID has invalid value: {cookie_str} ({e})"
            );
            return None;
        }
    };
    if pid as i32 != sys::env::getpid() {
        return None;
    }
    let sock_short = sys::env::getenv(c"FDSHELL_SOCKET")?;
    sys::ImportedFd::from_shortcstr(&sock_short).ok()
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
