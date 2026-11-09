// mutants: -C --lib -C --test -C block_bodies -C --test -C block_closers -C --test -C block_conditions -C --test -C block_redirect -- -E 'test(/^parse::block_tail::/) | binary(block_bodies) | binary(block_closers) | binary(block_conditions) | binary(block_redirect)'

//! The **glue-split pair** of the keyword block's tail (task #409): the closer
//! keyword can be glued to a redirection operator (`done>f`, `done&>f`), and the
//! tail walk reads operators only as whole tokens, so the glued word is split off
//! the closer first — with its mask and its zero-byte quote spans shifted to the
//! word's own start, the alignment `envassign::value_spans` applies to an
//! `=`-prefixed value. Extracted from `parse/block_tail.rs` for the line budget
//! (STYLE §2.2); the closer rule itself (`is_closer`, `closer_index`) stays with
//! the walk that consumes it.

use crate::parse::Token;
use alloc::vec;
use alloc::vec::Vec;

/// The tail tokens, with a glued closer split off: `done>f` contributes the
/// `>f` token, aligned with its raw range, mask and quote spans — the rule
/// `group_block::split_close` applies to the group's `}`.
pub(crate) fn split_close(kw: &[u8], at_closer: &[Token]) -> Vec<Token> {
    let rest_tokens = at_closer.get(1..).unwrap_or(&[]).to_vec();
    let Some(Token {
        word: t,
        start,
        end,
        fully_quoted: fq,
        mask,
        spans,
    }) = at_closer.first().cloned()
    else {
        return rest_tokens;
    };
    let Some(rest) = t
        .get(kw.len()..)
        .filter(|r| r.starts_with(b"<") || r.starts_with(b">"))
    else {
        return rest_tokens;
    };
    let off = kw.len();
    let mut tail = vec![Token {
        word: rest,
        start: start + off,
        end,
        fully_quoted: fq,
        mask: mask.get(off..).unwrap_or(&[]).to_vec(),
        spans: shift_spans(&spans, off),
    }];
    tail.extend_from_slice(&rest_tokens);
    tail
}

/// The quote-span indices of the slice of a word that starts at offset `off`:
/// every span at or past `off` shifts by it (`envassign::value_spans`'
/// alignment, without its `=`-byte step).
pub(crate) fn shift_spans(spans: &[usize], off: usize) -> Vec<usize> {
    spans
        .iter()
        .filter(|&&i| i >= off)
        .map(|&i| i - off)
        .collect()
}
