use crate::error::parse::ParseError;
use crate::redirect::RedirectDirection;
use error_stack::{Report, ensure};
use sys::ShortCStr;

/// Interpret the operator byte `dir` with the text after it: `>|` clobbers
/// (bypasses `noclobber`), `>>` appends, `<>` reads and writes, a bare `<`
/// reads, a bare `>` writes. Returns the path with the operator bytes consumed.
pub(super) fn redirect_op(
    dir: u8,
    after_op: ShortCStr,
) -> Result<(ShortCStr, RedirectDirection), Report<ParseError>> {
    if dir == b'>' && after_op.starts_with(b"|") {
        // A `>|` with nothing after is a bare operator token (`is_bare` matches
        // the suffix `>|`), so it is rejected by `bare::parse_bare` and never
        // reaches here: `after_op` always has the target.
        let r = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        Ok((r, RedirectDirection::Clobber))
    } else if dir == b'>' && after_op.starts_with(b">") {
        let r = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        ensure!(
            !(r.is_empty() || r.starts_with(b"%")),
            ParseError::InvalidRedirect
        );
        Ok((r, RedirectDirection::Append))
    } else if dir == b'<' && after_op.starts_with(b">") {
        let r = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        ensure!(!r.is_empty(), ParseError::InvalidRedirect);
        Ok((r, RedirectDirection::Rw))
    } else if dir == b'<' {
        Ok((after_op, RedirectDirection::Read))
    } else {
        Ok((after_op, RedirectDirection::Write))
    }
}
