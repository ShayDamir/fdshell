// mutants: -C --lib -C --test -C redirect -- -E 'test(/^parse::/) | binary(redirect)'

//! The parse layer's quote gates that read the **raw script text** (`line`):
//! the keyword gate, the definer gate and the ONE operator-word predicate. The
//! string-level half of the span test — "the span is longer than the word's own
//! bytes" — is a property of the string type, so it lives on
//! `ShortCStr::is_shorter_than_span` (task #545), and the gate that needs no
//! `line` — the separator span gate — is the token's own
//! [`Token::span_word`] (task #550); what this
//! module owns is the readings that combine the gate with the script text.
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
///
/// Stays free: it reads the raw script text `line` through [`quoted_word`], so its
/// data is not the token's own state (task #550).
pub(crate) fn keyword_word(tok: &Token, line: &[u8], kw: &[u8]) -> bool {
    let Token { word: t, end, .. } = tok;
    t.eq_bytes(kw) && !quoted_word(line, *end)
}

/// The definer-word gate: the word's raw span carries no quote byte, by **both**
/// tests — `ShortCStr::is_shorter_than_span` (the span is longer than the word's
/// own bytes) and [`quoted_word`]'s scan-back, because a definer word can end in
/// the POSIX `(` (`foo'('`) and the scan alone never reaches the quote bytes
/// behind it.
///
/// Stays free: it reads the raw script text `line` through [`quoted_word`] — the
/// `(`-ending definer word is why the scan-back is needed at all (task #550).
pub(crate) fn gated_word(tok: &Token, line: &[u8]) -> bool {
    let Token {
        word: t,
        start,
        end,
        ..
    } = tok;
    !t.is_shorter_than_span(*start, *end) && !quoted_word(line, *end)
}

/// The ONE **standalone operator word** predicate (task #524): `tok` is the
/// operator word `w` of [`THE_SET`] when [`Token::span_word`]'s gate passes — text
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

/// The word-slot rule (task **#470**): [`operator_word`]'s verdict **plus** the
/// tokenizer's `(`-word reading — the tokenizer keeps `(` a word byte, so a word
/// that *opens* with it is that control operator (the two signals
/// [`Token::take_open_paren`]/`stray::opens_groups` already read). Each half
/// carries the verdict on its own: a word-opening `(abc` never reaches the set,
/// and a bare `)` never opens with `(`; the quoted, escaped and mid-word
/// spellings answer `false` through both (the pins name each row). The word
/// slots that call this are the `case` word and the `for` word list; each brings
/// only its own owned set.
pub(crate) fn slot_rejects(tok: &Token, line: &[u8], owned: &dyn Fn(&[u8]) -> bool) -> bool {
    tok.opens_with_open_paren() || operator_word(tok, line, owned).is_some()
}

/// The ONE set: [`separator_shape`]'s words (task #396's one set, reached through
/// [`crate::keywords`]) plus the pipeline `|`, which stays [`pipe_token_span`]'s byte
/// (task #261's ONE predicate). The list spells the set out so the predicate can
/// hand back a `&'static` word; [`is_the_word`] reads each row from its owner.
const THE_SET: [&[u8]; 8] = [b"", b";", b"&&", b"||", b"&", b"(", b")", b"|"];

/// `w` is a row of [`THE_SET`] and `tok` **is** it: the row's own owner answers
/// ([`separator_shape`], or [`pipe_token_span`] for the `|`), then [`Token::span_word`]'s
/// gate.
///
/// Stays free: the `|` row reads the raw script text `line` through
/// [`pipe_token_span`], so its data is not the token's own state (task #550).
fn is_the_word(tok: &Token, line: &[u8], w: &[u8]) -> bool {
    let Token {
        word: t,
        start,
        end,
        ..
    } = tok;
    let in_the_set = if w == b"|" {
        pipe_token_span(line, *start, *end)
    } else {
        separator_shape(w) && t.eq_bytes(w)
    };
    in_the_set && tok.span_word(w)
}

#[cfg(test)]
mod tests;
