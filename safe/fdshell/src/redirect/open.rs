#![forbid(unsafe_code)]

use super::RedirectDef;
use crate::error::redirect::OpenRedirectError;
use sys::LocalFd;

pub fn open_redirect_files(redirects: &[RedirectDef]) -> Result<Vec<LocalFd>, OpenRedirectError> {
    let mut fds = Vec::new();
    for r in redirects {
        if let super::RedirectSource::Path(path) = &r.source {
            let name = sys::RefCStr::from(path.clone());
            fds.push(sys::openat2::open(&name, r.direction.open_flags())?);
        }
    }
    Ok(fds)
}
