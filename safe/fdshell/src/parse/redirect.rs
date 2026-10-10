mod bare;
pub(crate) use bare::clobber_prefix;

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::{RedirectDef, RedirectSource};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(super) fn parse_fd(prefix: &ShortCStr, dir: u8) -> Option<i32> {
    if prefix.is_empty() {
        Some(match dir {
            b'<' => 0,
            _ => 1,
        })
    } else {
        prefix.parse().ok()
    }
}

/// A redirect token: a bare operator (`>`, `>>`, `<`, `<>`, with an optional
/// numeric fd prefix) takes the next token as its path operand; an attached
/// operator is parsed in place. Returns the redirect and how many following
/// tokens the operator consumes (1 for the bare form).
pub fn parse_redirect(
    tokens: &[Token],
    i: usize,
) -> Result<Option<(RedirectDef, usize)>, Report<ParseError>> {
    let Some((t, _start, _end, fq, mask)) = tokens.get(i) else {
        return Ok(None);
    };
    if *fq {
        return Ok(None);
    }
    if bare::is_bare(t) {
        return bare::parse_bare(tokens, i).map(|def| def.map(|d| (d, 1)));
    }
    Ok(attached(t, mask)?.map(|def| (def, 0)))
}

/// The attached form: the operator byte is followed by its target inside the
/// same token (`>file`, `2>&1`, `>|file`, `3<%var`). A token whose first
/// `>`/`<` is its last byte is bare and is handled before this.
fn attached(s: &ShortCStr, mask: &[bool]) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let bytes = s.as_bytes().change_context(ParseError::Never)?;
    let op_pos = match bytes.iter().position(|&b| b == b'>' || b == b'<') {
        Some(p) => p,
        None => return Ok(None),
    };
    let dir = match bytes.get(op_pos) {
        Some(&d) => d,
        None => return Ok(None),
    };
    let after_op = match s.get(op_pos + 1..) {
        Some(r) => r,
        None => return Ok(None),
    };
    let prefix = match s.get(..op_pos) {
        Some(p) => p,
        None => return Ok(None),
    };
    if after_op.starts_with(b"&") {
        return super::fd_dup::parse_fd_dup_redirect(&after_op, &prefix, dir);
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
    let (rest, direction) = super::redirect_op::redirect_op(dir, after_op)?;
    let Some(export_to) = parse_fd(prefix, dir) else {
        return Ok(None);
    };
    if rest.starts_with(b"%") {
        let name = rest.get(1..).ok_or(ParseError::InvalidRedirect)?;
        // The direction is inert for a var source (resolve clones the table fd),
        // so the operator's direction is kept here only to carry `>|` through.
        return Ok(Some(RedirectDef {
            export_to,
            direction,
            source: RedirectSource::var(name),
        }));
    }
    if let Some(n) = super::fd_path::fd_path_target(&rest) {
        return Ok(Some(RedirectDef::dup(export_to, n)));
    }
    // The path is the token suffix after the operator; align its quote mask.
    let path_offset = op_pos + 1 + (after_op_len - rest.len());
    let path_mask = mask.get(path_offset..).unwrap_or(&[]).to_vec();
    Ok(Some(RedirectDef {
        export_to,
        direction,
        source: RedirectSource::path(rest, path_mask),
    }))
}
