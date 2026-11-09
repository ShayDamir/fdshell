// mutants: -C --lib -C --test -C semi_semi -- -E 'test(/^parse::semi::/) | binary(semi_semi)'

//! The closing-keyword search of `parse/semi.rs`, gated by the shared
//! command-position rule (task #236) and the `case`/`for` word-list rule (task
//! #273, [`List`]); the word rules it reads live in [`word`], and the walk they
//! feed in [`walk`] — shared with the `case` clause walk (task #418), whose only
//! difference is its stop predicate.

mod walk;
mod word;

pub(crate) use walk::walk_until;
pub(crate) use word::{List, glue_at};

use super::Token;
use crate::error::parse::ParseError;
use crate::keywords::{Closer, Keyword, keyword_kind};
use error_stack::{Report, bail};

/// The index of the block's closing keyword: the token at which the block's
/// pairing (seeded with `open`, the closer the block requires) first empties.
/// Tokens
/// after it are heredoc body data (a condition-position heredoc's bodies sit
/// in the block text after the closing keyword), not block structure.
///
/// `open` is the **kind** half (task #309): the walk seeds its pairing with the
/// closer this block requires, so a closer of another kind written in the body
/// is a word and the walk reaches the block's real closer (`while …; fi` never
/// ends a `while` body, `case x in done) …` never ends a `case`).
///
/// The scan carries bash's command-position rule over the token list
/// ([`crate::keywords::after`]/[`crate::keywords::scores`], task #236): a block
/// keyword written as an argument
/// (`echo done` in a `do` body) never closes the block — the `;` before a
/// real closer is what restarts the flag, and [`crate::keywords::separator_data`]
/// keeps a quoted
/// `";"` from being that restart (task #336): bash reads
/// `while :; do echo a ";" done` with `done` an **argument**, so the block stays
/// open and the byte-level scan's unclosed-block report is the verdict both
/// levels agree on. Braces stay ungated, as every
/// scorer of this rule leaves a literal `{`/`}` argument to the accepted
/// divergence `tests/group.rs::literal_brace_argument_in_a_block_body` pins.
pub(crate) fn closing_keyword_index(tokens: &[Token], line: &[u8]) -> Option<usize> {
    let open = opened_closer(tokens, line);
    closing_index(tokens, line, open, List::Plain)
}

/// The closer the block starting at token 0 requires (task #309): the opener
/// word's own kind, read from the one recognition table. A token list that
/// opens with no block keyword (the group's `{`, whose `Opens(Brace)` this
/// reads, and the arith tail a caller seeds by hand) pairs with `}`.
fn opened_closer(tokens: &[Token], line: &[u8]) -> Closer {
    tokens
        .first()
        .and_then(|Token { word: t, end, .. }| {
            t.as_bytes()
                .ok()
                .and_then(|word| keyword_kind(word, line, *end))
        })
        .map_or(Closer::Brace, |k| match k {
            Keyword::Opens(closer) | Keyword::Closes(closer) => closer,
        })
}

/// [`closing_keyword_index`] for a block that owns a word list, with the list
/// read as bash reads it ([`List`]): the `case` clause patterns and the `for`
/// words are word data, so their closer-shaped words never close the block.
/// `open` is that block's own closer (task #309).
pub(crate) fn list_closing_keyword_index(
    tokens: &[Token],
    line: &[u8],
    open: Closer,
    list: List,
) -> Option<usize> {
    closing_index(tokens, line, open, list)
}

/// The walk of [`walk`], stopped where this search stops: the moment the
/// pairing seeded with `open` empties (the walk's seed depth is then 1, so the
/// predicate is `depth == 0`), reading from token 1 — the block's own opener
/// word is never a candidate closer.
fn closing_index(tokens: &[Token], line: &[u8], open: Closer, list: List) -> Option<usize> {
    walk_until(tokens, line, Some(open), list, 1, &|_, depth, _| depth == 0)
}

/// The block's tokens up to and including its closing keyword; `err` when the
/// walk never empties its pairing. `open` is the closer this block requires
/// (task #309), so the walk bails out with the builder's own message instead of
/// ending at a closer of another kind. In a single-line block the heredoc bodies
/// sit in the block text **after** the closer, so the tokens past it are body
/// data, not block structure: every block builder truncates with this one rule,
/// and the word-list rule of a builder that owns a list goes in with it.
pub(crate) fn up_to_closing<'a>(
    tokens: &'a [Token],
    line: &[u8],
    open: Closer,
    list: List,
    err: ParseError,
) -> Result<&'a [Token], Report<ParseError>> {
    let Some(idx) = closing_index(tokens, line, open, list) else {
        bail!(err);
    };
    Ok(tokens.get(..=idx).ok_or(err)?)
}

#[cfg(test)]
mod tests;
