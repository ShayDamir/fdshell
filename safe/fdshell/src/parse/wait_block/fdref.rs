//! The `wait` pattern's `%` fd/task reference: `%name`, `%name&task`, `%name[]`.

use super::FdRef;
use crate::error::parse::ParseError;
use error_stack::{Report, ensure};
use sys::ShortCStr;

/// A `%`-prefixed fd/task reference: `%name` (a variable), `%name&task` (a
/// task of that variable), or `%name[]` (the whole array).
pub(super) fn parse_fdref(tok: &ShortCStr) -> Result<FdRef, Report<ParseError>> {
    let rest = tok.strip_prefix(b"%").ok_or(ParseError::WaitFdRefPercent)?;
    if let Some(task) = rest.strip_prefix(b"&") {
        ensure!(!task.is_empty(), ParseError::WaitMissingFd);
        return Ok(FdRef::Task(task));
    }
    if rest.ends_with(b"[]") {
        let base = rest
            .get(..rest.len() - 2)
            .ok_or(ParseError::WaitMissingFd)?;
        ensure!(!base.is_empty(), ParseError::WaitMissingFd);
        return Ok(FdRef::Array(base));
    }
    ensure!(!rest.is_empty(), ParseError::WaitMissingFd);
    Ok(FdRef::Var(rest))
}
