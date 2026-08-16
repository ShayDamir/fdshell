use error_stack::{Report, ResultExt, bail};
use sys::fcntl::O_PATH;
use sys::{LocalFd, ShortCStr};

use crate::error::child_process::ChildProcessError;

fn search_path(bin: &ShortCStr) -> Result<LocalFd, Report<ChildProcessError>> {
    let path_str = sys::env::getenv(c"PATH").unwrap_or(c"/usr/local/bin:/usr/bin:/bin".into());
    let slash: ShortCStr = c"/".into();
    for dir in path_str.split(b':') {
        if dir.is_empty() {
            continue;
        }
        let pathname = ShortCStr::concat(&[&dir, &slash, bin]);
        if let Ok(fd) = sys::openat2::open(pathname.export(), O_PATH) {
            return Ok(fd);
        }
    }
    bail!(ChildProcessError::NotFound(bin.clone()))
}

pub fn resolve_path(bin: &ShortCStr) -> Result<LocalFd, Report<ChildProcessError>> {
    if bin.contains(b'/') {
        sys::openat2::open(bin.export(), O_PATH)
            .change_context_lazy(|| ChildProcessError::NotFound(bin.clone()))
    } else {
        search_path(bin)
    }
}
