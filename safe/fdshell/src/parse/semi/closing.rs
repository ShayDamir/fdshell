//! The closing-keyword search of `parse/semi.rs`, gated by the shared
//! command-position rule (task #236).

use super::Token;
use crate::keywords::{Glue, Place, after, keyword_delta, pair_payload, scores};

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
    let mut depth: i32 = 1;
    let mut place = Place::Command;
    for (i, (t, _, end, _, _)) in tokens.iter().enumerate().skip(1) {
        let Some(word) = t.as_bytes().ok() else {
            continue;
        };
        // The tokenizer splits a glued `&&`/`||` into two operator tokens; the
        // first is the pair's head, the second its tail — the same glue the
        // byte-level scan computes from the line's bytes (task #236's S2).
        let glue = glue_at(tokens, i, word);
        // A glued cond pair *inside* a token (`a||if` is one token) hides its
        // payload from the token walk; [`keyword_delta`]'s pair-tail rule
        // scores it, so the `if` nests and its `fi` closes *the if*, not the
        // enclosing block (task #236's S2).
        let scored = scores(word, line, *end, place);
        place = after(word, line, *end, place, glue);
        // A glued cond pair *inside* a token (`a||if` is one token) hides its
        // payload from the token walk: score the payload at the place the
        // pair sets, the same payload rule the byte scans apply via [`Glue`]
        // (task #236's S2: `do echo a||if :; …; fi` nests the `if`, so its
        // `fi` closes *the if* and the span scan keeps the enclosing block's
        // closer).
        if let Some(payload) = pair_payload(word)
            && scores(payload, line, *end, place)
        {
            let delta = keyword_delta(payload, line, *end).unwrap_or(0);
            depth += delta;
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

/// Where token `i` starts in a cond pair ([`Glue`]): the tokenizer splits a
/// glued pair into two tokens, so the pair's head and tail are the tokens
/// adjacent to the run — the byte-level scan sees the same two word starts
/// because [`is_word_break`](crate::scan::advance) breaks words at operator
/// bytes.
fn glue_at(tokens: &[Token], i: usize, word: &[u8]) -> Glue {
    if !is_op(word) {
        return Glue::None;
    }
    let same = |j: usize| {
        tokens
            .get(j)
            .is_some_and(|(t, _, _, _, _)| t.as_bytes().is_ok_and(|p| p == word))
    };
    match (same(i - 1), same(i + 1)) {
        (true, _) => Glue::Tail,
        (false, true) => Glue::Head,
        _ => Glue::None,
    }
}

/// A cond/pipeline operator token.
fn is_op(word: &[u8]) -> bool {
    matches!(word, b"|" | b"&")
}

#[cfg(test)]
mod tests;
