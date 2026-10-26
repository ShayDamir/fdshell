//! The `&`-terminator **flag** of a condition slice (task #356): the cut `&` is
//! the job's fact, and this predicate is its single owner — kept apart from the
//! slicing rules in `trim.rs`.

use super::Token;

/// Whether a condition slice ends in the plain `&` that terminates the compound
/// list (task #356): `trim_condition` **cuts** that `&` from the condition
/// text — the condition runs as its own command list, whose parse layer rejects
/// a bare `&` — so the returned flag is the fact's single owner, handed to
/// `run_cond_list`'s existing `background` argument by `if_exec`/`loop_`.
/// Trailing `;` **tokens** whose byte is a newline (or `)`) step back: bash
/// reads `if true &` newline `then` exactly like `if true & then` (measured). A
/// `;` **byte** stops the walk — that slice is the condition's own parse error
/// and must stay one (task #355). The byte test is `amp_semi`'s: the
/// tokenizer emits a `;` token for a `;` byte, a newline and `)` alike.
pub(crate) fn condition_background(tokens: &[Token], line: &[u8]) -> bool {
    let mut end = tokens.len();
    while end > 0 && is_newline_semi(tokens.get(end - 1), line) {
        end -= 1;
    }
    tokens
        .get(end.saturating_sub(1))
        .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"&"))
}

/// The `;` **token** points at a newline (or `)`) byte, so it is a newline in
/// disguise, never the `;` that ends a condition (`false` for a word and for a
/// `;` byte, `amp_semi`'s pair arm).
fn is_newline_semi(tok: Option<&Token>, line: &[u8]) -> bool {
    tok.is_some_and(|(t, start, _, _, _, _)| t.eq_bytes(b";") && line.get(*start) != Some(&b';'))
}
