use super::{Redirect, RedirectDef};
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use alloc::vec::Vec;
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
                    .ok_or(OpenRedirectError::Open)?
                    .try_clone_above(
                        r.export_to
                            .checked_add(1)
                            .ok_or(OpenRedirectError::FdNumberOutOfRange)?,
                    )
                    .change_context(OpenRedirectError::Open)?,
                super::RedirectSource::Path(_) => opened_iter
                    .next()
                    .ok_or(OpenRedirectError::Open)?
                    .try_clone()
                    .change_context(OpenRedirectError::Open)?,
            };
            Ok(r.resolve(local))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn out_of_range_export_to_is_actionable_error() {
        let mut state = ShellState::new();
        let fd = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
        state.fds.insert(c"x".into(), fd);
        let report = resolve_redirects(&[RedirectDef::var(i32::MAX, c"x")], &[], &state)
            .err()
            .unwrap();
        assert!(matches!(
            report.current_context(),
            OpenRedirectError::FdNumberOutOfRange
        ));
    }
}
