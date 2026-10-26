use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use crate::parse::redirect::{fd_var_operand, quoted_bytes};
use crate::redirect::RedirectDirection;
use error_stack::{Report, bail, ensure};
use sys::ShortCStr;

/// Interpret the operator byte `dir` with the text after it: `>|` clobbers
/// (bypasses `noclobber`), `>>` appends, `<>` reads and writes, a bare `<`
/// reads, a bare `>` writes. Returns the path with the operator bytes
/// consumed. The continuation byte is an operator byte only while it is
/// **unquoted** (`after_mask` is the tail's quote mask, the gate's own
/// `quoted_bytes`): a quoted `|`/`>` ends the operator at `dir`, and the
/// tail from that byte becomes the path (POSIX XCU 2.9.1, bash parity).
pub(super) fn redirect_op(
    dir: u8,
    after_op: ShortCStr,
    after_mask: &[MaskBit],
) -> Result<(ShortCStr, RedirectDirection), Report<ParseError>> {
    let quoted_next = quoted_bytes(after_mask, 0, 1);
    if !quoted_next && dir == b'>' && after_op.starts_with(b"|") {
        // A `>|` with nothing after is a bare operator token (`is_bare` matches
        // the suffix `>|`), so it is rejected by `bare::parse_bare` and never
        // reaches here: `after_op` always has the target.
        let r = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        Ok((r, RedirectDirection::Clobber))
    } else if !quoted_next && dir == b'>' && after_op.starts_with(b">") {
        let r = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        // An empty tail is the bare `>>` arm, and `>>%var` is the append-to-fd
        // form this operator does not have. The `%` is that form's trigger only
        // while **unquoted**: the operand's first byte is `after_op[1]`, i.e.
        // index `1` of `after_mask`, so `>>"%v"` appends to the file `%v`
        // (bash parity, task #329).
        if r.is_empty() || fd_var_operand(&r, after_mask, 1) {
            bail!(ParseError::InvalidRedirect);
        }
        Ok((r, RedirectDirection::Append))
    } else if !quoted_next && dir == b'<' && after_op.starts_with(b">") {
        let r = after_op.get(1..).ok_or(ParseError::InvalidRedirect)?;
        ensure!(!r.is_empty(), ParseError::InvalidRedirect);
        Ok((r, RedirectDirection::Rw))
    } else if dir == b'<' {
        Ok((after_op, RedirectDirection::Read))
    } else {
        Ok((after_op, RedirectDirection::Write))
    }
}

#[cfg(test)]
mod tests;
