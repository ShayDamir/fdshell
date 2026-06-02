#![forbid(unsafe_code)]

use super::RedirectDef;
use sys::LocalFd;

pub fn open_redirect_files(redirects: &[RedirectDef]) -> Result<Vec<LocalFd>, i32> {
    let mut fds = Vec::new();
    for r in redirects {
        if let super::RedirectSource::Path(path) = &r.source {
            let name = path.to_c_string()?;
            fds.push(sys::openat2::open(&name, r.direction.open_flags())?);
        }
    }
    Ok(fds)
}
