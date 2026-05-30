use sys::LocalFd;

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
    sys::ImportedFd::from_bytes(sys::shellfd::SHELLFD_STR).ok()
}

pub fn init_shellfd() -> Result<FdShellMode, i32> {
    if let Some(dupfd) = detect_nested() {
        let fd = dupfd.try_into_local()?;
        Ok(FdShellMode::Nested(fd))
    } else {
        let fd = sys::shellfd::reserve_shellfd()?;
        Ok(FdShellMode::Standalone(fd))
    }
}
