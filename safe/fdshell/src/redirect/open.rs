#![forbid(unsafe_code)]

use super::RedirectDef;
use sys::LocalFd;

pub fn open_redirect_files(redirects: &[RedirectDef]) -> Result<Vec<LocalFd>, i32> {
    redirects
        .iter()
        .filter_map(|r| match &r.source {
            super::RedirectSource::Path(path) => {
                let name = path.to_c_string();
                Some((name, r.direction.open_flags()))
            }
            _ => None,
        })
        .map(|(name, flags)| sys::openat2::open(&name, flags))
        .collect()
}
