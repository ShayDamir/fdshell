// mutants: -C --lib -C --test -C redirect -- -E 'test(/^parse::/) | binary(redirect)'

//! The parse layer's quote gate on a word's **raw** span `(start, end)`. The
//! string-level half of the test — "the span is longer than the word's own
//! bytes" — is a property of the string type, so it lives on
//! `ShortCStr::is_shorter_than_span` (task #545); what this module owns is the
//! parse reading of it, because the span here is the word's *raw* span and so
//! carries bytes the word dropped (quotes, escapes).
//!
//! An empty quoted region contributes no bytes to the word (and its per-byte
//! mask), so "this word was quoted" is not representable in the mask — it has
//! to be derived from the span. The same test is true for escaped bytes
//! (`"a\"b"`), which is intended: bash keeps such a word whole when its
//! expansion is empty, too.

use super::{Token, pipe_token_span};
use crate::keywords::{quoted_word, separator_shape};

/// The keyword-recognition gate (POSIX §2.9.1, task #247): `tok` *is* the
/// keyword word `kw` only when the word carries no quote byte. Token-level
/// twin of the gate `keywords::keyword_delta` applies to block openers and
/// closers — both read [`quoted_word`] on the same raw span, so the
/// block-depth scan and the block builders cannot disagree. Not the per-byte
/// mask (`""if` has no bit) and not `fq` (`i"f"` is not fully quoted): bash
/// runs both as command words.
pub(crate) fn keyword_word(tok: &Token, line: &[u8], kw: &[u8]) -> bool {
    let (t, _start, end, _fq, _mask, _spans) = tok;
    t.eq_bytes(kw) && !quoted_word(line, *end)
}

/// The definer-word gate: the word's raw span carries no quote byte, by **both**
/// tests — `ShortCStr::is_shorter_than_span` (the span is longer than the word's
/// own bytes) and [`quoted_word`]'s scan-back, because a definer word can end in
/// the POSIX `(` (`foo'('`) and the scan alone never reaches the quote bytes
/// behind it.
pub(crate) fn gated_word(tok: &Token, line: &[u8]) -> bool {
    let (t, start, end, _fq, _mask, _spans) = tok;
    !t.is_shorter_than_span(*start, *end) && !quoted_word(line, *end)
}

/// The separator-recognition gate (POSIX §2.9.1, task #336): `tok` **is** the
/// separator word `w` only when its raw span is exactly those bytes, so it
/// carries no quote byte. A quote-protected `";"`, `';'`, `"|"`, `"&"`, `")"`
/// is a word whose *text* is the separator, and bash reads it as word **data**:
/// it splits no statement, no pipeline, no `case` clause, and it never restarts
/// command position (`echo a ";" b` prints `a ; b`).
/// LESSONS: "A separator-shaped word needs the span gate"
///
/// The gate is the raw span, never the mask (`"";` is an empty word token
/// **plus** a real `;`, and `a";"b` is one word whose text is `a;b`) and never
/// [`quoted_word`]'s scan-back, whose word-break set contains the separator
/// bytes themselves, so it is sound only for a keyword word, which ends in a
/// letter. The span form is also the gate the sites that own no `line` can
/// compute (`semi/trim.rs::is_semi`, `pipeline/stage.rs::trim_at_semi`,
/// `envassign.rs`). Escape spellings need no arm: `\;`, `\|`, `\)` keep the pair
/// in the token **text** (`parse/token/escape.rs`), so no separator text exists
/// to gate. Byte-level twin: the byte scanners read raw bytes, where a quoted
/// separator's word text is `";"` / `"|"` and no separator row can fire —
/// `parse::redirect::clobber::pipe::pipe_token_span` is this rule for the `|`.
pub(crate) fn span_word(tok: &Token, w: &[u8]) -> bool {
    let (t, start, end, _fq, _mask, _spans) = tok;
    t.eq_bytes(w) && !t.is_shorter_than_span(*start, *end)
}

/// The ONE **standalone operator word** predicate (task #524): `tok` is the
/// operator word `w` of [`THE_SET`] when [`span_word`]'s gate passes — text
/// equality plus the raw span — so a quoted `"&&"`, `'&'`, `\&` or glued `x&&`
/// is word **data** and never the operator. `owned` is the per-site variation the
/// rule takes as data: it says which of those words **this** grammar position
/// owns as its own separator, and a word the position owns is not rejected here.
/// `Some(word)` is the operator word a site must report.
///
/// Each site supplies only its own owned set — the `for` word list owns the
/// `;`/newline that terminates it before `do`, the `case` pattern list owns the
/// `|`, `(`, `)`, `;` its own grammar puts in a word slot — and every other
/// operator word that lands in a word slot is rejected (tasks #495/#496: each
/// site that listed only the words it happened to notice is one bug).
pub(crate) fn operator_word(
    tok: &Token,
    line: &[u8],
    owned: &dyn Fn(&[u8]) -> bool,
) -> Option<&'static [u8]> {
    THE_SET
        .into_iter()
        .find(|w| is_the_word(tok, line, w) && !owned(w))
}

/// The ONE set: [`separator_shape`]'s words (task #396's one set, reached through
/// [`crate::keywords`]) plus the pipeline `|`, which stays [`pipe_token_span`]'s byte
/// (task #261's ONE predicate). The list spells the set out so the predicate can
/// hand back a `&'static` word; [`is_the_word`] reads each row from its owner.
const THE_SET: [&[u8]; 8] = [b"", b";", b"&&", b"||", b"&", b"(", b")", b"|"];

/// `w` is a row of [`THE_SET`] and `tok` **is** it: the row's own owner answers
/// ([`separator_shape`], or [`pipe_token_span`] for the `|`), then [`span_word`]'s
/// gate.
fn is_the_word(tok: &Token, line: &[u8], w: &[u8]) -> bool {
    let (t, start, end, _fq, _mask, _spans) = tok;
    let in_the_set = if w == b"|" {
        pipe_token_span(line, *start, *end)
    } else {
        separator_shape(w) && t.eq_bytes(w)
    };
    in_the_set && span_word(tok, w)
}

#[cfg(test)]
mod tests;
