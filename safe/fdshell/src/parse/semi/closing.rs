//! The closing-keyword search of `parse/semi.rs`, gated by the shared
//! command-position rule (task #236) and the `case`/`for` word-list rule (task
//! #273, [`List`]); the word rules it reads live in [`word`].

mod word;

pub(crate) use word::{List, glue_at};
use word::{next_word, scores_at};

use super::Token;
use crate::error::parse::ParseError;
use crate::keywords::{
    Closer, Keyword, Pairing, Place, after, keyword_kind, pair_payload, scores, separator_data,
    word_place,
};
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
/// ([`after`]/[`scores`], task #236): a block keyword written as an argument
/// (`echo done` in a `do` body) never closes the block — the `;` before a
/// real closer is what restarts the flag, and [`separator_data`] keeps a quoted
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
        .and_then(|(t, _, end, _, _, _)| {
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

fn closing_index(tokens: &[Token], line: &[u8], open: Closer, list: List) -> Option<usize> {
    let mut pairing = Pairing::new();
    pairing.open(open);
    let mut place = Place::Command;
    for (i, tok) in tokens.iter().enumerate().skip(1) {
        let (t, _start, end, _fq, _mask, _spans) = tok;
        // Defensive: the tokens were built from `line`, so their bytes are
        // always valid; an error token means a future tokenizer that hands
        // this scan a token it cannot read, and the scan skips it.
        let Some(word) = t.as_bytes().ok() else {
            continue;
        };
        // The tokenizer splits a glued `&&`/`||` into two operator tokens; the
        // first is the pair's head, the second its tail — the same glue the
        // byte-level scan computes from the line's bytes (task #236's S2).
        let glue = glue_at(tokens, i, word);
        let scored = scores_at(word, line, *end, place, list, next_word(tokens, i));
        // The gate reads the pairing **before** this word is applied: the
        // closer's kind must match the block it closes, not the one the word
        // itself leaves open behind it (task #309).
        // A separator-shaped **word** leaves the place an ordinary word leaves
        // ([`word_place`]: an argument, a `for` list stays a list), never the
        // command restart the separator earns (task #336).
        place = if separator_data(tok) {
            word_place(place)
        } else {
            after(word, line, *end, place, glue, pairing.top())
        };
        // A glued cond pair *inside* a token (`a||if` is one token) hides its
        // payload from the token walk: score the payload at the place the pair
        // sets, the same payload rule the byte scans apply via [`Glue`] (task
        // #236's S2: `do echo a||if :; …; fi` nests the `if`, so its `fi`
        // closes *the if* and the walk keeps the enclosing block's closer).
        if let Some(payload) = pair_payload(word)
            && scores(payload, line, *end, place)
        {
            pairing.apply(payload, line, *end);
            if !pairing.is_open() {
                return Some(i);
            }
        }
        if scored {
            pairing.apply(word, line, *end);
            if !pairing.is_open() {
                return Some(i);
            }
        }
    }
    None
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
