//! The ONE owner of "the byte offset at which a redirect **operand** begins in
//! this token". A pre-parse word rewriter needs exactly that offset to judge
//! the operand on its own (POSIX XCU 2.7 `worddef`: the operand expands to one
//! word), and after its text rewrite every parse-level offset has moved, so the
//! rule lives next to the parser's own owners: `bare`'s suffix test, the
//! `quoted_bytes` gate, `token_fd`'s io-number rule and `redirect_op`'s
//! continuation bytes — nothing is re-derived here, and
//! `tests::the_operand_slice_is_the_operand_the_parser_derives` pins the two
//! owners agreeing on the same spelling (LESSONS: a byte-level and a
//! token-level rule must call ONE predicate).

use crate::mask::MaskBit;
use crate::parse::Token;
use crate::parse::redirect::bare;
use crate::parse::redirect::{quoted_bytes, token_fd};
use crate::parse::redirect_op::redirect_op;
use sys::ShortCStr;

/// The offset of token `i`'s redirect operand inside the token's own text.
/// `None` when the token has no operand in itself: not a redirect at all (a
/// quoted operator byte, a non-digit io-number prefix), a `<<`/`<<<` word
/// (`brace_expand/protect.rs` owns those), a `&>&name`/`&>|&name` pidvar, a
/// bare operator, whose operand is the **next** token (claimed at that token's
/// offset 0), or an operator with nothing after it.
pub(crate) fn operand_offset(tokens: &[Token], i: usize) -> Option<usize> {
    let (t, _start, _end, _fq, mask, _spans) = tokens.get(i)?;
    bg_operand(t, mask)
        .or_else(|| attached_operand(t, mask))
        .or_else(|| bare_operand(tokens, i))
}

/// The `&>` family, whose operator starts with `&` and so is invisible to the
/// io-number rule: `&>word` / `&>>word` (path or `%var`) put the operand one
/// past the operator's last byte — the same index `bg_redirect`'s `op_end` gate
/// and `path_offset` use, one shape, two owners pinned by the agreement test.
/// The pidvar arms carry no operand, and a bare `&>` / `&>>` takes the next
/// token.
fn bg_operand(t: &ShortCStr, mask: &[MaskBit]) -> Option<usize> {
    let rest = t.strip_prefix(b"&>")?;
    if rest.starts_with(b"&") || rest.starts_with(b"|") {
        return None;
    }
    let op_end = 2 + usize::from(rest.starts_with(b">"));
    if quoted_bytes(mask, 0, op_end) || op_end >= t.len() {
        return None;
    }
    Some(op_end)
}

/// The attached `>`/`<` arms, walked in the parser's own arm order: the quoting
/// gate on the operator byte, the here-doc operator, the io-number prefix, then
/// the dup arm (`<&word` / `>&word`, its `&` unquoted too) and the path/`%var`
/// arms, whose offset is one past the operator's last byte — the very
/// `path_offset` the path word is sliced at.
fn attached_operand(t: &ShortCStr, mask: &[MaskBit]) -> Option<usize> {
    let bytes = t.as_bytes().ok()?;
    let op_pos = bytes.iter().position(|&b| b == b'>' || b == b'<')?;
    if quoted_bytes(mask, op_pos, op_pos + 1) {
        return None;
    }
    let dir = bytes.get(op_pos).copied()?;
    let prefix = t.get(..op_pos)?;
    let after_op = t.get(op_pos + 1..)?;
    if dir == b'<' && after_op.starts_with(b"<") {
        return None;
    }
    // The io-number rule, used only as the gate: a non-digit prefix means the
    // word is no redirect at all, so it has no operand.
    token_fd(&prefix, mask, dir)?;
    let after_op_len = after_op.len();
    let dup = after_op.starts_with(b"&") && !quoted_bytes(mask, op_pos + 1, op_pos + 2);
    let off = if dup {
        op_pos + 2
    } else {
        let after_mask = mask.get(op_pos + 1..).unwrap_or(&[]);
        let rest = redirect_op(dir, after_op, after_mask).ok()?;
        op_pos + 1 + (after_op_len - rest.0.len())
    };
    (off < t.len()).then_some(off)
}

/// The bare forms: token `i` is the operand of the bare operator at `i - 1`, so
/// its operand starts at offset 0. The operator gates are the parsers' own
/// (`bare::parse_bare` and `bg_redirect`'s bare arm), so a token is claimed as
/// an operand exactly where a parser reads an operand.
fn bare_operand(tokens: &[Token], i: usize) -> Option<usize> {
    let (t, _start, _end, _fq, mask, _spans) = tokens.get(i.checked_sub(1)?)?;
    (is_bare_operator(t, mask) || is_bare_bg_operator(t, mask)).then_some(0)
}

/// A bare `>`-family operator token: `bare::is_bare`'s suffix test plus the
/// quoting and io-number gates of `bare::parse_bare`.
fn is_bare_operator(t: &ShortCStr, mask: &[MaskBit]) -> bool {
    let Some(op) = bare::operator_suffix(t) else {
        return false;
    };
    if !bare::is_bare(t) || quoted_bytes(mask, t.len() - op.len(), t.len()) {
        return false;
    }
    let dir = if op.eq_bytes(b"<") || op.eq_bytes(b"<>") {
        b'<'
    } else {
        b'>'
    };
    t.get(..t.len() - op.len())
        .is_some_and(|prefix| token_fd(&prefix, mask, dir).is_some())
}

/// A bare `&>` / `&>>` operator token (`bg_redirect`'s bare arm): the operator
/// is the whole word, unquoted.
fn is_bare_bg_operator(t: &ShortCStr, mask: &[MaskBit]) -> bool {
    let Some(rest) = t.strip_prefix(b"&>") else {
        return false;
    };
    let op_end = 2 + usize::from(rest.starts_with(b">"));
    op_end == t.len() && !quoted_bytes(mask, 0, op_end)
}

#[cfg(test)]
mod tests;
