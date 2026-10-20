//! The attached redirect form: the operator's target sits inside the same
//! token (`>file`, `2>&1`, `>|file`, `3<%var`).

use crate::error::parse::ParseError;
use crate::parse::fd_dup::parse_fd_dup_redirect;
use crate::parse::fd_path::fd_path_target;
use crate::parse::redirect_op::redirect_op;
use crate::redirect::{RedirectDef, RedirectSource};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

/// The attached form: the operator byte is followed by its target inside the
/// same token (`>file`, `2>&1`, `>|file`, `3<%var`). A token whose first
/// `>`/`<` is its last byte is bare and is handled before this. A **quoted**
/// first operator byte ends the operator (POSIX XCU 2.9.1 reads only
/// unquoted bytes): the token is an ordinary argument, as bash prints it.
pub(super) fn attached(
    s: &ShortCStr,
    mask: &[bool],
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let bytes = s.as_bytes().change_context(ParseError::Never)?;
    let op_pos = match bytes.iter().position(|&b| b == b'>' || b == b'<') {
        Some(p) => p,
        None => return Ok(None),
    };
    // The gate: a quoted first operator byte is word data, not an operator.
    if crate::parse::redirect::quoted_bytes(mask, op_pos, op_pos + 1) {
        return Ok(None);
    }
    let Some(dir) = bytes.get(op_pos).copied() else {
        return Ok(None);
    };
    let Some(after_op) = s.get(op_pos + 1..) else {
        return Ok(None);
    };
    let Some(prefix) = s.get(..op_pos) else {
        return Ok(None);
    };
    // A `<` whose next byte is another `<` is the here-doc/here-string
    // operator, never a path redirect: the path operand of `3<<T` is not `<T`
    // (the operator's own second byte). A `<<`-shaped token the here-doc
    // machinery does not claim (its rule: `scan::heredoc::io::span_operator`)
    // is word data, like every other unsplit word.
    if dir == b'<' && after_op.starts_with(b"<") {
        return Ok(None);
    }
    // The dup arm needs the `&` unquoted too: a quoted `&` ends the operator,
    // so `2> "&" 1` falls through to the path arm and opens the file `&1`
    // (bash parity, R1's continuation clause).
    if after_op.starts_with(b"&")
        && !crate::parse::redirect::quoted_bytes(mask, op_pos + 1, op_pos + 2)
    {
        return parse_fd_dup_redirect(&after_op, &prefix, mask, dir);
    }
    parse_path_redirect(s, mask, op_pos, dir, &prefix)
}

fn parse_path_redirect(
    s: &ShortCStr,
    mask: &[bool],
    op_pos: usize,
    dir: u8,
    prefix: &ShortCStr,
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let after_op = s.get(op_pos + 1..).ok_or(ParseError::InvalidRedirect)?;
    let after_op_len = after_op.len();
    // The operator tail's mask: `redirect_op` gates its `>|`/`>>`/`<>`
    // continuation bytes on it (a quoted continuation byte ends the operator
    // there, and `rest` becomes the path from the operator's second byte).
    let after_mask = mask.get(op_pos + 1..).unwrap_or(&[]);
    let (rest, direction) = redirect_op(dir, after_op, after_mask)?;
    let Some(export_to) = super::token_fd(prefix, mask, dir) else {
        return Ok(None);
    };
    if rest.starts_with(b"%") {
        let name = rest.get(1..).ok_or(ParseError::InvalidRedirect)?;
        // The direction is inert for a var source (resolve clones the table fd),
        // so the operator's direction is kept here only to carry `>|` through.
        return Ok(Some(RedirectDef {
            export_to,
            extra_to: None,
            direction,
            source: RedirectSource::var(name),
        }));
    }
    if let Some(n) = fd_path_target(&rest) {
        return Ok(Some(RedirectDef::dup(export_to, n)));
    }
    // The path is the token suffix after the operator; align its quote mask.
    // `path_offset` is exactly the index one past the operator's last byte —
    // the gate's `to` bound (one owner of that offset, LESSONS).
    let path_offset = op_pos + 1 + (after_op_len - rest.len());
    let path_mask = mask.get(path_offset..).unwrap_or(&[]).to_vec();
    Ok(Some(RedirectDef {
        export_to,
        extra_to: None,
        direction,
        source: RedirectSource::path(rest, path_mask),
    }))
}

#[cfg(test)]
mod tests;
