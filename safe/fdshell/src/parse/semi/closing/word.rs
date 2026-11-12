// mutants: -C --lib -C --test -C semi_semi -- -E 'test(/^parse::semi::/) | binary(semi_semi)'

//! The **word** rules the closing-keyword walk reads: how a token's word scores
//! as a block keyword ([`scores_at`], with the `case`/`for` word-list rule
//! [`List`], whose `for` half is the walk's **seed** [`seed_place`], task
//! #522), and what a token's neighbours say about it ([`glue_at`],
//! [`next_word`]).

use super::Token;
use crate::keywords::{Glue, Place, scores};
use crate::parse::delimiter_token_indices;
use crate::scan::heredoc::terminator_spans;
use alloc::vec::Vec;

/// How the walk reads a keyword closer written inside a `case`/`for **word
/// list** ([`Place::Pattern`], task #236): bash reads a word list as words.
/// [`List::Words`] decides that as the place the walk is **seeded** with
/// ([`seed_place`], task #522) — the word-list rule of [`scores`], read through
/// the place, as its module doc says; only the `case` list keeps its own arm.
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
    /// there — `for x in done; do echo x; done` iterates over the word `done`,
    /// and so does the name slot behind `for` (task #522): the list is the
    /// place the walk starts in, [`seed_place`].
    Words,
}

/// The place the walk starts in — the byte-level scan's seed rule, read by the
/// token walk for the first time (task #522): the `for` run is seeded in
/// [`Place::Words`] ([`crate::segment::block`]'s `list_seed`, task #306), so
/// every word behind the opener — the **name** slot included — is list data,
/// and a closer-shaped name never closes the block. The plain and `case` walks
/// keep their command-position seed.
pub(super) fn seed_place(list: List) -> Place {
    match list {
        List::Plain | List::Case => Place::Command,
        List::Words => Place::Words,
    }
}

/// May `word` be scored as a block keyword at `place`? This is [`scores`]
/// unchanged — its place gate already owns the `for` word list, whose rule the
/// walk reads through the [`seed_place`] it starts in (task #522) — except for
/// the `case` list, where [`List`] decides. A `case` `esac` that a pattern's `)`
/// follows is a **pattern word** (`case x in esac) cat <<T;; esac` is the clause
/// `esac) …`), so only an `esac` no `)` follows terminates a list.
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
        List::Plain | List::Words => scores(word, line, end, place),
        List::Case => word == b"esac" && next != Some(b")"),
    }
}

/// Which tokens of `tokens` are **here-doc structure**: an operator's delimiter
/// word ([`delimiter_token_indices`]) and every word whose span starts inside a
/// terminator line ([`terminator_spans`], tabs included so the `<<-` form is
/// covered) — the same set `brace_expand/protect` protects (task #266). Such a
/// word never scores as brace syntax, so a closer-shaped delimiter word
/// (`cat <<}`) never closes the block it writes its body in; the walk asks once
/// per walk, and the word stays a token (its words are what the verbatim
/// families measure) and still moves the place.
pub(super) fn structure_flags(line: &[u8], tokens: &[Token]) -> Vec<bool> {
    let delimiters = delimiter_token_indices(line, tokens);
    let terminators = terminator_spans(line);
    tokens
        .iter()
        .enumerate()
        .map(|(i, Token { start, .. })| delimiters.contains(&i) || covered(&terminators, *start))
        .collect()
}

/// Whether `start` sits inside one of the terminator-line spans `spans`.
fn covered(spans: &[(usize, usize)], start: usize) -> bool {
    spans.iter().any(|&(s, e)| start >= s && start < e)
}

/// Where token `i` starts in a cond pair ([`Glue`]): the tokenizer splits a
/// glued pair into two tokens, so the pair's head and tail are the tokens
/// adjacent to the run — the byte-level scan sees the same two word starts
/// because [`is_word_break`](crate::scan::advance) breaks words at operator
/// bytes.
///
/// Both neighbour rows go through [`Token::span_word`] (task #530, bug #474): the pair
/// is made of **unquoted** operator bytes (POSIX §2.9.1), so a quote-protected
/// `"|"`/`"&"` word standing next to a real one is word data and is no pair
/// half — `echo "|" | cat` is `echo`'s argument through a pipeline, not a `||`
/// pair. Escape spellings need no arm: `\|`/`\&` keep the pair in the word text,
/// so no operator text exists to compare, and a quoted *current* word is caught
/// by the walk's `separator_data` row before this row is consulted.
pub(crate) fn glue_at(tokens: &[Token], i: usize, word: &[u8]) -> Glue {
    if !is_op(word) {
        return Glue::None;
    }
    let same = |j: usize| tokens.get(j).is_some_and(|tok| tok.span_word(word));
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
        .and_then(|Token { word: t, .. }| t.as_bytes().ok())
}

/// A cond/pipeline operator token.
pub(super) fn is_op(word: &[u8]) -> bool {
    matches!(word, b"|" | b"&")
}
