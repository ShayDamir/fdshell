use error_stack::{Report, ResultExt};
use sys::LocalFd;

use crate::error::shell::ShellInitError;

pub enum FdShellMode {
    Nested(LocalFd),
    Standalone(LocalFd),
}

fn detect_nested() -> Option<sys::ImportedFd> {
    let cookie = std::env::var("FDSHELL_CAPTURE").ok();
    let pid = cookie.and_then(|s| s.parse::<u32>().ok())?;
    if pid != std::process::id() {
        return None;
    }
    sys::ImportedFd::try_from(sys::shellfd::SHELLFD_STR).ok()
}

pub fn init_shellfd() -> Result<FdShellMode, Report<ShellInitError>> {
    if let Some(dupfd) = detect_nested() {
        let fd = dupfd
            .try_into_local()
            .change_context(ShellInitError::NestedFd)?;
        sys::shellfd::set_capture_active(true);
        Ok(FdShellMode::Nested(fd))
    } else {
        let fd = sys::shellfd::reserve_shellfd().change_context(ShellInitError::ShellSocket)?;
        Ok(FdShellMode::Standalone(fd))
    }
}
