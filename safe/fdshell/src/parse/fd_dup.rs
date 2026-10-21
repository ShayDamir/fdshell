use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use crate::redirect::RedirectDef;
use error_stack::Report;
use sys::ShortCStr;

use super::redirect::token_fd;

/// Parse `>&N` (dup), `>&-` (close), and `>&%var` (dup from an fd variable)
/// redirections; other `&…` tails are not redirects. The io-number prefix
/// goes through `token_fd`: only **unquoted** digits select a descriptor,
/// so a quoted `'2'>&1` dups at the operator's default fd 1.
pub fn parse_fd_dup_redirect(
    after_op: &ShortCStr,
    prefix: &ShortCStr,
    mask: &[MaskBit],
    dir: u8,
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let Some(export_to) = token_fd(prefix, mask, dir) else {
        return Ok(None);
    };
    let rest = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
    if rest.eq_bytes(b"-") {
        return Ok(Some(RedirectDef::close(export_to)));
    }
    match rest.parse::<i32>() {
        Ok(from) if from >= 0 => Ok(Some(RedirectDef::dup(export_to, from))),
        _ => Ok(rest
            .strip_prefix(b"%")
            .map(|name| RedirectDef::dup_var(export_to, name))),
    }
}
