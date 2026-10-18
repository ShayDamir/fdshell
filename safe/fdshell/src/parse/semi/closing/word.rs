//! The **word** rules the closing-keyword walk reads: how a token's word scores
//! as a block keyword ([`scores_at`], with the `case`/`for` word-list rule
//! [`List`]), and what a token's neighbours say about it ([`glue_at`],
//! [`next_word`]).

use super::Token;
use crate::keywords::{Glue, Place, scores};

/// How the walk reads a keyword closer written inside a `case`/`for **word
/// list** ([`Place::Pattern`], task #236): bash reads a word list as words.
#[derive(Clone, Copy)]
pub(crate) enum List {
    /// Not a word-list walk: [`scores`]' rule unchanged (a closer scores in a
    /// `case` list, so `case x in esac` is a complete `case`).
    Plain,
    /// The `case` list: `esac` is its terminator, so it scores; every other
    /// closer-shaped word is a **pattern** — `case x in done) echo p;; esac`
    /// matches the word `done` (task #273).
    Case,
    /// The `for` list: it is closed by `do`, a non-closer, so no closer scores
    /// there — `for x in done; do echo x; done` iterates over the word `done`.
    Words,
}

/// May `word` be scored as a block keyword at `place`? Outside a word list this
/// is [`scores`] unchanged; inside one, [`List`] decides. A `case` `esac` that a
/// pattern's `)` follows is a **pattern word** (`case x in esac) cat <<T;; esac`
/// is the clause `esac) …`), so only an `esac` no `)` follows terminates a list.
pub(crate) fn scores_at(
    word: &[u8],
    line: &[u8],
    end: usize,
    place: Place,
    list: List,
    next: Option<&[u8]>,
) -> bool {
    if !matches!(place, Place::Pattern) {
        return scores(word, line, end, place);
    }
    match list {
        List::Plain => scores(word, line, end, place),
        List::Case => word == b"esac" && next != Some(b")"),
        List::Words => false,
    }
}

/// Where token `i` starts in a cond pair ([`Glue`]): the tokenizer splits a
/// glued pair into two tokens, so the pair's head and tail are the tokens
/// adjacent to the run — the byte-level scan sees the same two word starts
/// because [`is_word_break`](crate::scan::advance) breaks words at operator
/// bytes.
pub(crate) fn glue_at(tokens: &[Token], i: usize, word: &[u8]) -> Glue {
    if !is_op(word) {
        return Glue::None;
    }
    let same = |j: usize| {
        tokens
            .get(j)
            .is_some_and(|(t, _, _, _, _, _)| t.as_bytes().is_ok_and(|p| p == word))
    };
    match (same(i - 1), same(i + 1)) {
        (true, _) => Glue::Tail,
        (false, true) => Glue::Head,
        _ => Glue::None,
    }
}

/// The word of the token after `i`, for the rules that read a following token
/// (a `case` pattern's `)`); `None` at the end of the list.
pub(crate) fn next_word(tokens: &[Token], i: usize) -> Option<&[u8]> {
    tokens
        .get(i + 1)
        .and_then(|(t, _, _, _, _, _)| t.as_bytes().ok())
}

/// A cond/pipeline operator token.
pub(super) fn is_op(word: &[u8]) -> bool {
    matches!(word, b"|" | b"&")
}
