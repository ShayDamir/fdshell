#![forbid(unsafe_code)]

use super::{Redirect, RedirectDef};
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::LocalFd;

pub fn resolve_redirects(
    redirects: &[RedirectDef],
    opened: &[LocalFd],
    state: &ShellState,
) -> Result<Vec<Redirect>, Report<OpenRedirectError>> {
    let mut opened_iter = opened.iter();
    redirects
        .iter()
        .map(|r| {
            let local = match &r.source {
                super::RedirectSource::Var(var) => state
                    .fds
                    .get(var)
                    .ok_or(OpenRedirectError)?
                    .try_clone_above(r.export_to + 1)
                    .change_context(OpenRedirectError)?,
                super::RedirectSource::Path(_) => opened_iter
                    .next()
                    .ok_or(OpenRedirectError)?
                    .try_clone()
                    .change_context(OpenRedirectError)?,
            };
            Ok(r.resolve(local))
        })
        .collect()
}
