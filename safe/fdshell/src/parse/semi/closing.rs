//! The closing-keyword search of `parse/semi.rs`, gated by the shared
//! command-position rule (task #236) and the `case`/`for` word-list rule (task
//! #273, [`List`]); the word rules it reads live in [`word`].

mod word;

pub(crate) use word::List;
use word::{glue_at, next_word, scores_at};

use super::Token;
use crate::error::parse::ParseError;
use crate::keywords::{Place, after, keyword_delta, pair_payload, scores};
use error_stack::{Report, bail};

/// The index of the block's closing keyword: the token at which the block
/// depth (starting at 1 for the opening keyword) first returns to 0. Tokens
/// after it are heredoc body data (a condition-position heredoc's bodies sit
/// in the block text after the closing keyword), not block structure.
///
/// The scan carries bash's command-position rule over the token list
/// ([`after`]/[`scores`], task #236): a block keyword written as an argument
/// (`echo done` in a `do` body) never closes the block — the `;` before a
/// real closer is what restarts the flag. Braces stay ungated, as every
/// scorer of this rule leaves a literal `{`/`}` argument to the accepted
/// divergence `tests/group.rs::literal_brace_argument_in_a_block_body` pins.
pub(crate) fn closing_keyword_index(tokens: &[Token], line: &[u8]) -> Option<usize> {
    closing_index(tokens, line, List::Plain)
}

/// [`closing_keyword_index`] for a block that owns a word list, with the list
/// read as bash reads it ([`List`]): the `case` clause patterns and the `for`
/// words are word data, so their closer-shaped words never close the block.
pub(crate) fn list_closing_keyword_index(
    tokens: &[Token],
    line: &[u8],
    list: List,
) -> Option<usize> {
    closing_index(tokens, line, list)
}

fn closing_index(tokens: &[Token], line: &[u8], list: List) -> Option<usize> {
    let mut depth: i32 = 1;
    let mut place = Place::Command;
    for (i, (t, _, end, _, _, _)) in tokens.iter().enumerate().skip(1) {
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
        place = after(word, line, *end, place, glue);
        // A glued cond pair *inside* a token (`a||if` is one token) hides its
        // payload from the token walk: score the payload at the place the pair
        // sets, the same payload rule the byte scans apply via [`Glue`] (task
        // #236's S2: `do echo a||if :; …; fi` nests the `if`, so its `fi`
        // closes *the if* and the walk keeps the enclosing block's closer).
        if let Some(payload) = pair_payload(word)
            && scores(payload, line, *end, place)
        {
            depth += keyword_delta(payload, line, *end).unwrap_or(0);
            if depth == 0 {
                return Some(i);
            }
        }
        let Some(delta) = scored.then(|| keyword_delta(word, line, *end)).flatten() else {
            continue;
        };
        depth += delta;
        if depth == 0 {
            return Some(i);
        }
    }
    None
}

/// The block's tokens up to and including its closing keyword; `err` when the
/// walk never returns to depth 0. In a single-line block the heredoc bodies sit
/// in the block text **after** the closer, so the tokens past it are body data,
/// not block structure: every block builder truncates with this one rule, and
/// the word-list rule of a builder that owns a list goes in with it.
pub(crate) fn up_to_closing<'a>(
    tokens: &'a [Token],
    line: &[u8],
    list: List,
    err: ParseError,
) -> Result<&'a [Token], Report<ParseError>> {
    let Some(idx) = closing_index(tokens, line, list) else {
        bail!(err);
    };
    Ok(tokens.get(..=idx).ok_or(err)?)
}

#[cfg(test)]
mod tests;
