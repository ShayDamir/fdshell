//! The **path** arm of the attached form: the operator's operand is a file
//! word or an fd-variable `%var` target, read through the surfaces `attached`
//! delegates to — `path_word::suffix`, `fd_var_operand`, `token_fd` — whose
//! owners keep the offset rules this arm consumes (LESSONS).

// mutants: -C --lib -C --test -C redirect -- -E 'test(/^parse::/) | binary(redirect)'

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::redirect::path_word::suffix;
use crate::parse::redirect::{fd_var_operand, token_fd};
use crate::parse::redirect_op::redirect_op;
use crate::redirect::{RedirectDef, RedirectSource};
use error_stack::Report;
use sys::ShortCStr;

pub(super) fn parse_path_redirect(
    t: &Token,
    op_pos: usize,
    dir: u8,
    prefix: &ShortCStr,
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let (s, _start, _end, _fq, mask, _spans) = t;
    let after_op = s.get(op_pos + 1..).ok_or(ParseError::InvalidRedirect)?;
    let after_op_len = after_op.len();
    // The operator tail's mask: `redirect_op` gates its `>|`/`>>`/`<>`
    // continuation bytes on it (a quoted continuation byte ends the operator
    // there, and `rest` becomes the path from the operator's second byte).
    let after_mask = mask.get(op_pos + 1..).unwrap_or(&[]);
    let (rest, direction) = redirect_op(dir, after_op, after_mask)?;
    let Some(export_to) = token_fd(prefix, mask, dir) else {
        return Ok(None);
    };
    // The path is the token suffix after the operator, with its mask and its
    // zero-byte quote spans sliced at the same offset (`path_word`, one owner
    // of that offset with the here-string word, LESSONS). `path_offset` is
    // exactly the index one past the operator's last byte — the gate's `to`
    // bound, and the mask index of the operand's first byte (the `%` arm).
    let path_offset = op_pos + 1 + (after_op_len - rest.len());
    if fd_var_operand(&rest, mask, path_offset) {
        let name = rest.get(1..).ok_or(ParseError::InvalidRedirect)?;
        // The direction is inert for a var source (resolve clones the table fd),
        // so the operator's direction is kept here only to carry `>|` through.
        // The name is the whole token tail (`>%v"w"` names `vw`): the parse-time
        // reading, pinned by the unit tests (the word path's region-capped
        // `collect_name` is a different surface).
        return Ok(Some(RedirectDef {
            export_to,
            extra_to: None,
            direction,
            source: RedirectSource::var(name),
        }));
    }
    // A quoted `%` target fell through the mask gate above and lands here:
    // the plain file word (`>"%v"` is the file `%v`, bash parity, #329).
    Ok(Some(RedirectDef {
        export_to,
        extra_to: None,
        direction,
        source: suffix(t, path_offset)?,
    }))
}
