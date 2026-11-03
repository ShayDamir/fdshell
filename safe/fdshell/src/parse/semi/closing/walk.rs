//! The **walk** shared by the closing-keyword search of [`super`] and the `case`
//! clause's `;;` window search (task #418): one loop over the token list, the
//! same command-position/pairing state machine the other scorers read (LESSONS
//! "One clause rule, three scorers"). The caller supplies the **stop predicate**;
//! the two differ in nothing else — the closing search stops where the pairing
//! empties, the clause walk stops at a `;;` window written **at its own level**
//! (a `;;` inside a nested block belongs to that block, so the arm it ends is
//! the nested one).
//!
//! `open` is the seed half (task #309): the closer of the block the caller is
//! already inside, so `Some` seeds the pairing and the walk starts at that depth
//! (`1`), `None` starts it at the top level (`0`) — the stray-paren walk's case,
//! which never walks this loop. `start` is the first token the walk reads, so the
//! closing search keeps skipping its opener word (token 0) while an arm walk
//! reads its own first token: it can itself be an opener (`a) case b in …`).

use super::Token;
use super::word::{List, glue_at, next_word, scores_at};
use crate::keywords::{
    Closer, Glue, Pairing, Place, after, pair_payload, scores, separator_data, word_place,
};

/// Walk `tokens` from `start` and answer the first index at which `stop` accepts
/// the walk's state: `(index, depth, seed_depth)`. The predicate is asked at
/// every moment the pairing can have moved (a glued payload's apply, the word's
/// own apply) and once at the end of every token, so each caller's stop test
/// sees exactly the state its own rule reads — and the pairing gate of the
/// closer row is read **before** the word, as always.
pub(crate) fn walk_until(
    tokens: &[Token],
    line: &[u8],
    open: Option<Closer>,
    list: List,
    start: usize,
    stop: &dyn Fn(usize, usize, usize) -> bool,
) -> Option<usize> {
    let mut pairing = Pairing::new();
    let seed = match open {
        Some(closer) => {
            pairing.open(closer);
            1
        }
        None => 0,
    };
    let mut place = Place::Command;
    for (i, tok) in tokens.iter().enumerate().skip(start) {
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
        // Index 0 has no predecessor token: [`glue_at`] reads `tokens[i - 1]`,
        // so an arm walk that starts at 0 hands it `Glue::None` (the same guard
        // `parse/case_clause/stray.rs` keeps).
        let glue = if i == 0 {
            Glue::None
        } else {
            glue_at(tokens, i, word)
        };
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
            // The payload's answer is read **before** the word's own score is
            // applied, because one word can be both: `{&&fi` scores as the
            // group opener and its payload `fi` is the closer that empties a
            // walk seeded with the `if` (the pin
            // `closing_index_walks_the_block_depth` stops there — the scan the
            // payload rule was written for).
            if stop(i, pairing.depth(), seed) {
                return Some(i);
            }
        }
        if scored {
            pairing.apply(word, line, *end);
        }
        // The test runs on **every** token, not only on the scored ones: the
        // clause walk's `;;` window sits on separator words, which score
        // nothing (task #418). A scored word's answer is the same one the arm
        // above reads, so the closing search is untouched by it.
        if stop(i, pairing.depth(), seed) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests;
