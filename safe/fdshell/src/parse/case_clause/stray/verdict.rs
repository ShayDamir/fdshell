// mutants: -C --lib -C --test -C case -- -E 'test(/^parse::case_clause::/) | binary(case)'

//! What a judged depth-0 `)` token means (task #300 and task #419). The walk
//! in [`super`] finds the candidate and asks this module; the caller owns the
//! messages (STYLE §4.7). Each arm is measured against bash 5.3.9; the place
//! machine that reads the word is the shared one (LESSONS "One clause rule,
//! three scorers").

use super::Token;
use super::opens_groups;
use crate::keywords::Place;
use crate::mask::{MaskBit, quoted_at};
use crate::parse::word_quoted::span_word;

/// The closers a `((` word **outside** the arm region owns (task #419): when
/// the clause's first pattern word was the production opener — its leading `(`
/// unquoted, the byte `take_open_paren` strips (task #334) — and its bytes
/// started with `((`, the arithmetic region owns two closers. The production
/// consumed the first (the `)` token right before the region); the region walk
/// starts owning the other, so `case abc in ((a*)) echo m;; *) echo n;; esac`
/// keeps parsing (bash rejects it, an accepted pin) while `case abc in (a*) )
/// echo m;; *) echo n;; esac` is refused. `body_start` is the region's start.
pub(crate) fn arith_seed(tokens: &[Token], body_start: usize) -> usize {
    let (Some(word), Some(closer)) = (
        tokens.get(body_start.wrapping_sub(2)),
        tokens.get(body_start.wrapping_sub(1)),
    ) else {
        return 0;
    };
    // `take_open_paren`'s discriminator: only an unquoted leading `(` is the
    // token POSIX writes; the closer is the pattern walk's span-clean `)`.
    let arms_pair = |tok: &Token| {
        let Token {
            word: text, mask, ..
        } = tok;
        text.as_bytes()
            .is_ok_and(|b| b.starts_with(b"((") && !mask.is_empty() && !quoted_at(mask, 0))
    };
    usize::from(arms_pair(word) && span_word(closer, b")"))
}

/// Why the walk judged a depth-0 `)` token; the caller owns the messages
/// ([`crate::error::parse::ParseError`], STYLE §4.7).
#[derive(Debug, PartialEq)]
pub(crate) enum Stray {
    /// task #300: the pattern paren of the clause the arm is missing its `;;`
    /// before — a command-position predecessor word that is not an operator
    /// token
    ClausePattern,
    /// task #419: a paren the arm's command list cannot take (bash rc 2) —
    /// behind an argument, behind an operator token, or with no word before it
    CloseParen,
}

/// What the depth-0, unquoted, span-clean `)` token at index `i` means, judged
/// on the place that read its predecessor word — in this order, each arm
/// measured against bash 5.3.9: a **word-list place** is the arm's own pattern
/// paren ([`None`], clean); task #300's rule — a command-position predecessor
/// word that is not an operator token — is the clause pattern paren of the
/// missing `;;`; everything else at depth 0 (an argument predecessor, an
/// operator predecessor, or no predecessor at all) is a paren the command list
/// cannot take.
pub(crate) fn classify(before: Place, region: &[Token], i: usize) -> Option<Stray> {
    if matches!(before, Place::Pattern | Place::Words) {
        return None;
    }
    if is_a_clause_pattern(before, region, i) {
        return Some(Stray::ClausePattern);
    }
    Some(Stray::CloseParen)
}

/// [`opens_groups`] **gated on the word's place**: a `(` opens a group only in
/// command position, where bash's grammar recognises it as an operator (the
/// shared machine, LESSONS "One clause rule, three scorers"); an
/// argument-position word contributes no openers, so the `)` written behind
/// `echo (b` is the arm's stray paren (task #419). The gate is the call site's;
/// [`opens_groups`]'s own byte contract stays the counter's tests.
pub(crate) fn opens_groups_at(place: Place, bytes: &[u8], mask: &[MaskBit]) -> usize {
    if matches!(place, Place::Command) {
        opens_groups(bytes, mask)
    } else {
        0
    }
}

/// Is the `)` at index `i` the pattern paren of the clause that is missing its
/// `;;` before it (task #300)? Its predecessor word must sit in command position
/// (`b` in `a) b) ;;`, behind the `;` in `a) echo hi; b) ;;`) and must not be an
/// operator token — a `)` written behind `;`, `|`, `&` or another `)` closes
/// that operator, never a pattern list, and the paren lands in the other arm of
/// [`classify`]. The operator test is the **token**'s, gated on the raw span, so
/// a quoted `";"` / `"|"` predecessor is word data and leaves the paren behind
/// it an argument's paren (task #336). The `i > 0` guard belongs to this arm
/// only: with no predecessor token the walk has no word to read, and task
/// #419's rule takes that row.
fn is_a_clause_pattern(before: Place, region: &[Token], i: usize) -> bool {
    matches!(before, Place::Command)
        && i > 0
        && region.get(i - 1).is_some_and(|prev| !is_operator(prev))
}

/// The operator words the tokenizer emits as tokens of their own — every one of
/// them through [`span_word`], so a quoted spelling is a word, never an operator
/// predecessor (task #336).
fn is_operator(tok: &Token) -> bool {
    let w = |sep: &[u8]| span_word(tok, sep);
    w(b";") || w(b"|") || w(b"&") || w(b")")
}

#[cfg(test)]
mod tests;
