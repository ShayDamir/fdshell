//! Unit pins for the closing-keyword scan (task #236): the [`Glue`] the
//! tokenizer's operator tokens carry ([`glue_at`] — the head and the tail of a
//! split `||` pair, and no glue for a pipeline `|`, a `&`, or a word that is
//! not an operator), and the depth walk of [`closing_keyword_index`] over
//! nested blocks, glued closers and a glued pair's payload.

#![allow(clippy::unwrap_used)]

use super::{closing_keyword_index, glue_at, is_op};
use crate::keywords::Glue;
use crate::parse::Token;
use crate::parse::token::tokenize_statement;
use alloc::vec::Vec;

/// The tokens of `line`, as the parse layer hands them to the scan.
fn toks(line: &[u8]) -> Vec<Token> {
    tokenize_statement(line).unwrap()
}

/// The token index of `word`'s `n`th occurrence, for the glue rows.
fn nth(t: &[Token], word: &[u8], n: usize) -> usize {
    t.iter()
        .enumerate()
        .filter(|(_, (tok, _, _, _, _))| tok.as_bytes().is_ok_and(|w| w == word))
        .nth(n)
        .map(|(i, _)| i)
        .unwrap()
}

/// The tokenizer splits a spaced `||` pair into two operator tokens, so the
/// pair's head is the token whose *successor* is the same operator and its
/// tail is the token whose *predecessor* is. A pipeline `|`, a `&`, a `&&`
/// token (one token, not a split pair) and a word that is no operator at all
/// carry no glue — and an equal word that is not an operator never becomes a
/// pair half (`is_op` is the gate, not the adjacency).
#[test]
fn glue_at_names_the_split_pair_head_and_tail() {
    let t = toks(b"echo a || if :; then :; fi");
    assert_eq!(glue_at(&t, nth(&t, b"|", 0), b"|"), Glue::Head);
    assert_eq!(glue_at(&t, nth(&t, b"|", 1), b"|"), Glue::Tail);
    // A three-byte run: its first byte is the head, the rest are tails.
    let t = toks(b"echo a ||| if :; then :; fi");
    assert_eq!(glue_at(&t, nth(&t, b"|", 0), b"|"), Glue::Head);
    assert_eq!(glue_at(&t, nth(&t, b"|", 1), b"|"), Glue::Tail);
    assert_eq!(glue_at(&t, nth(&t, b"|", 2), b"|"), Glue::Tail);
    // A pipeline `|` is a single-byte run: no glue (#292's carve-out).
    let t = toks(b"echo a|if :; then :; fi");
    assert_eq!(glue_at(&t, nth(&t, b"|", 0), b"|"), Glue::None);
    // A `&` token with no equal neighbour: no glue.
    let t = toks(b"echo a & echo b");
    assert_eq!(glue_at(&t, nth(&t, b"&", 0), b"&"), Glue::None);
    // A `&&` token is one token, never a split pair.
    let t = toks(b"echo a && if :; then :; fi");
    assert_eq!(glue_at(&t, nth(&t, b"&&", 0), b"&&"), Glue::None);
    // An equal word that is no operator stays unglued: adjacency alone is not
    // a pair (the row that kills an `is_op` that answers `true` for any word).
    let t = toks(b"echo echo");
    assert_eq!(glue_at(&t, 1, b"echo"), Glue::None);
    assert!(is_op(b"|") && is_op(b"&"));
    assert!(!is_op(b"&&") && !is_op(b"echo") && !is_op(b""));
}

/// The depth walk: the scan returns the token where the block opened by token
/// 0 returns to depth 0. Nested blocks close at *their* closers; a closer
/// glued to an operator word (`fi&&echo`) is a closer; a `case` closes at
/// `esac`; a pair *payload* in command position nests, and the pair behind an
/// argument (`echo a&&if`) does not. The rows are the scan's own contract —
/// the end-to-end behaviour of these shapes is pinned against bash in
/// `tests/glued_pair.rs` and `tests/cond_blocks.rs`.
#[test]
fn closing_index_walks_the_block_depth() {
    // A one-level block: the closer is the last token.
    let line = b"if :; then :; fi";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(6));
    // Nested blocks: the outer closer, not the inner one (kills a `+=` that
    // becomes `-=` and a `depth == 0` test that becomes `!=`).
    let line = b"if :; then if :; then :; fi; fi";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(12));
    let line = b"if if :; then :; fi; fi";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(9));
    let line = b"while :; do :; done";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(6));
    let line = b"case x in a) :;; esac";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(8));
    // A closer glued to its operator word scores as the closer.
    let line = b"if :; then :; fi&&echo x";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(6));
    // A pair payload in command position nests: the group-opener word
    // `{&&if` puts its payload `if` in command position, so the block gains a
    // depth no closer of this statement closes — the scan finds no closing
    // keyword. Without the payload arm the two `fi`s would close the block.
    let line = b"if :; then {&&if :; then :; fi; fi";
    assert_eq!(closing_keyword_index(&toks(line), line), None);
    // The same pair behind an argument word stays data (#292's place rule):
    // the payload never nests, and the block closes at the first `fi`.
    let line = b"while :; do echo a&&if :; then :; fi; done";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(11));
    // The pair payload can be the *closer* itself: the group-opener word
    // `{&&fi` scores +1 as the opener and its payload `fi` closes the block,
    // so the scan stops at the pair word's own token, not at a later word.
    let line = b"if :; then {&&fi";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(4));
    // No closer at all: the block never returns to depth 0.
    let line = b"echo a || if :; then :; fi";
    assert_eq!(closing_keyword_index(&toks(line), line), None);
}
