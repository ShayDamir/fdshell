#![forbid(unsafe_code)]

use super::{Redirect, RedirectDef};
use crate::vars::FdVars;
use sys::LocalFd;

pub fn resolve_redirects<'a>(
    redirects: &[RedirectDef],
    opened: &'a [LocalFd],
    vars: &'a FdVars,
) -> Result<Vec<Redirect<'a>>, i32> {
    let mut opened_iter = opened.iter();
    redirects
        .iter()
        .map(|r| {
            let local = match &r.source {
                super::RedirectSource::Var(var) => vars.resolve(var).ok_or(sys::errno::EINVAL),
                super::RedirectSource::Path(_) => opened_iter.next().ok_or(sys::errno::EIO),
            }?;
            Ok(r.resolve(local))
        })
        .collect()
}
