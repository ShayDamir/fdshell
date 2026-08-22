use crate::error::parse::ParseError;
use crate::redirect::RedirectDef;
use error_stack::Report;
use sys::ShortCStr;

use super::redirect::parse_fd;

/// Parse `>&N` (dup) and `>&-` (close) redirections; other `&…` tails are not redirects.
pub fn parse_fd_dup_redirect(
    after_op: &ShortCStr,
    prefix: &ShortCStr,
    dir: u8,
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let Some(export_to) = parse_fd(prefix, dir) else {
        return Ok(None);
    };
    let rest = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
    if rest.eq_bytes(b"-") {
        return Ok(Some(RedirectDef::close(export_to)));
    }
    match rest.parse::<i32>() {
        Ok(from) if from >= 0 => Ok(Some(RedirectDef::dup(export_to, from))),
        _ => Ok(None),
    }
}
