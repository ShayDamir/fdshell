#![forbid(unsafe_code)]

use super::{Redirect, RedirectDef};
use crate::state::ShellState;
use sys::LocalFd;

pub fn resolve_redirects(
    redirects: &[RedirectDef],
    opened: &[LocalFd],
    state: &ShellState,
) -> Result<Vec<Redirect>, i32> {
    let mut opened_iter = opened.iter();
    redirects
        .iter()
        .map(|r| {
            let local = match &r.source {
                super::RedirectSource::Var(var) => state
                    .fds
                    .get(var)
                    .ok_or(sys::errno::EINVAL)?
                    .try_clone_above(r.export_to + 1)?,
                super::RedirectSource::Path(_) => {
                    opened_iter.next().ok_or(sys::errno::EIO)?.try_clone()?
                }
            };
            Ok(r.resolve(local))
        })
        .collect()
}
