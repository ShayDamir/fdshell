//! Unit pins for the closing-keyword scan (task #236): the [`Glue`] the
//! tokenizer's operator tokens carry ([`glue_at`] — the head and the tail of a
//! split `||` pair, and no glue for a pipeline `|`, a `&`, or a word that is
//! not an operator), and the depth walk of [`closing_keyword_index`] over
//! nested blocks, glued closers and a glued pair's payload.

#![allow(clippy::unwrap_used)]

use super::closing_keyword_index;
use super::walk::walk_until;
use super::word::{List, glue_at, is_op, structure_flags};
use crate::keywords::{Closer, Glue};
use crate::parse::Token;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;

/// The tokens of `line`, as the parse layer hands them to the scan.
fn toks(line: &[u8]) -> Vec<Token> {
    tokenize_statement(line).unwrap()
}

/// The token index of `word`'s `n`th occurrence, for the glue rows.
fn nth(t: &[Token], word: &[u8], n: usize) -> usize {
    t.iter()
        .enumerate()
        .filter(|(_, Token { word: tok, .. })| tok.as_bytes().is_ok_and(|w| w == word))
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
    // A pipeline `|` is a single-byte run: no glue (the word behind it is
    // the stage arm's, task #292).
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
    // The same pair behind an argument word stays data (the in-block place
    // rule, task #236): the payload never nests. Task #309's pairing decides
    // what the walk then closes: the seeded `while` wants `done`, so the
    // mismatched `fi` (token 11) is a word and the walk stops at the block's
    // own `done` (token 13) — the body keeps its tail (the end-to-end twin is
    // `tests/glued_pair.rs`, bash rc 0).
    let line = b"while :; do echo a&&if :; then :; fi; done";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(13));
    // The pair payload can be the *closer* itself: the group-opener word
    // `{&&fi` scores +1 as the opener and its payload `fi` closes the block,
    // so the scan stops at the pair word's own token, not at a later word.
    let line = b"if :; then {&&fi";
    assert_eq!(closing_keyword_index(&toks(line), line), Some(4));
    // No closer at all: the block never returns to depth 0.
    let line = b"echo a || if :; then :; fi";
    assert_eq!(closing_keyword_index(&toks(line), line), None);
}

/// Task #273: a `case` clause list and a `for` word list are **word** lists, so
/// their closer-shaped words never close the block. In a `case` list only `esac`
/// scores, and only when no pattern `)` follows it (`case x in esac` is a
/// complete block, `case x in esac) …` is a clause). A `for` list scores
/// nothing: it is closed by `do`.
///
/// The `plain` column is the walk **without** the list gate (the mutation the
/// [`List`] argument defends). Task #309's pairing covers two of these rows by
/// itself: the walk is seeded with the block's own closer, so a `done`/`fi`
/// written in a `case` list mismatches the `case`'s `esac` and the walk reaches
/// the real `esac` (token 9) — bash's read of `case x in done) …` (rc 0), part
/// of task **#310**'s list rule landing for free. The `esac`-shaped list word is
/// where the list gate stays the only defense: an `esac` matches, so the plain
/// walk still closes there.
#[test]
fn word_lists_are_walked_as_words() {
    let cases = [
        (
            b"case x in done) echo p;; esac".as_slice(),
            Some(9),
            Some(9),
        ),
        (b"case x in fi) echo p;; esac".as_slice(), Some(9), Some(9)),
        (b"case x in esac".as_slice(), Some(3), Some(3)),
        (
            b"case x in esac) echo p;; esac".as_slice(),
            Some(3),
            Some(9),
        ),
        (
            b"for x in done; do echo x; done".as_slice(),
            Some(3),
            // The `case` walk on a `for` text (seeded with `esac`, the shape
            // `case_block` calls it with) finds **no** closer: the pairing
            // answers a `for`'s `done` is not an `esac` (task #309), where the
            // type-blind arithmetic ended the walk at the loop's own `done`.
            None,
        ),
        (b"for x in a; do echo x; done".as_slice(), Some(9), None),
    ];
    // The `Words` column is decided by the walk's **seed** since task #522
    // (`seed_place`), not by an arm of `scores_at`: the same answers, read at
    // the place the list rule owns.
    for (line, plain, case) in cases {
        let t = toks(line);
        assert_eq!(closing_keyword_index(&t, line), plain, "{line:?}");
        assert_eq!(
            super::list_closing_keyword_index(&t, line, Closer::Esac, super::List::Case),
            case,
            "{line:?}"
        );
        if line.starts_with(b"for") {
            assert_eq!(
                super::list_closing_keyword_index(&t, line, Closer::Done, super::List::Words,),
                Some(9),
                "{line:?}"
            );
        }
    }
    // The nested half (task #306): a `for` in **command position** puts the
    // words behind it at [`Place::Words`], and the gate of [`scores`] gates the
    // list's words out — so the inner list's closer-shaped `done` is a word and
    // the walk closes the **enclosing** block at its own `done` (token 14). The
    // top-level rows above read the builder-side [`List`] rule; this row is the
    // place rule, and it needs no [`List`] argument at all.
    let line = b"while :; do for i in done; do :; done; done";
    let t = toks(line);
    assert_eq!(
        closing_keyword_index(&t, line),
        Some(14),
        "{line:?}: {}",
        t.len()
    );
}

/// Task #522: the byte-level scan seeds a `for` run in [`Place::Words`] so a
/// closer-shaped word of the list stays a word (task #306), and the token walk
/// now starts in that seed too — so **every** word behind the opener is list
/// data, the name slot included: POSIX #2.9.1 puts a *name* there, and a
/// closer-shaped name never closes the block. The `plain` column is the walk
/// without the seed (the mutation [`seed_place`] defends): it stops at the name
/// word itself. Every row is bash parity — bash 5.3.9 runs the loop (`[a]`/`x`,
/// rc 0), pinned end-to-end in `tests/loops.rs`; the e2e twins of the glued and
/// the `;` spellings are tasks #521/#416's.
#[test]
fn a_closer_shaped_name_of_a_for_block_is_walked_as_the_name() {
    for (line, plain, words) in [
        (
            b"for done in a; do echo x; done".as_slice(),
            Some(1),
            Some(9),
        ),
        (b"for done; do echo x; done".as_slice(), Some(1), Some(7)),
        (b"for done do echo x; done".as_slice(), Some(1), Some(6)),
    ] {
        let t = toks(line);
        assert_eq!(closing_keyword_index(&t, line), plain, "{line:?}");
        assert_eq!(
            super::list_closing_keyword_index(&t, line, Closer::Done, List::Words),
            words,
            "{line:?}"
        );
    }
}

/// Task #522's **fold is the rule at these rows**, not dead-code cleanup: the
/// merged arm of [`super::word::scores_at`] is [`crate::keywords::scores`] read
/// through the place, and it stays **live** past the block's own list — the walk
/// leaves [`Place::Words`] at the list's first separator and a nested `case` puts
/// it back at [`Place::Pattern`] through the `in` row, where a complete
/// `case x in esac` **scores**. So a nested `case` with an empty clause list
/// inside a `for` block completes and the block closes at its own `done`; every
/// row is bash parity (measured 2026-11-12, bash 5.3.9, rc 0), pinned
/// end-to-end in
/// `tests/loops.rs::a_nested_case_with_an_empty_clause_list_inside_a_for_block_runs_at_bash_parity`.
/// The `plain` column is the walk
/// without the list gate — equal by the fold — and the pre-fold arm
/// (`List::Words => false`, the mutation the fold deletes) left the nested `case`
/// unclosed: **no** closing keyword for the block.
#[test]
fn a_nested_case_with_an_empty_clause_list_completes_in_the_words_walk() {
    for (line, plain, words) in [
        (
            b"for x in a; do case x in esac; echo tail; done".as_slice(),
            Some(14),
            Some(14),
        ),
        (
            b"for x in a b; do case x in esac; echo tail; done".as_slice(),
            Some(15),
            Some(15),
        ),
        (
            b"for x in a; do true && case x in esac; echo tail; done".as_slice(),
            Some(16),
            Some(16),
        ),
        (
            b"for x in a; do { case x in esac; echo tail; }; done".as_slice(),
            Some(17),
            Some(17),
        ),
    ] {
        let t = toks(line);
        assert_eq!(closing_keyword_index(&t, line), plain, "{line:?}");
        assert_eq!(
            super::list_closing_keyword_index(&t, line, Closer::Done, List::Words),
            words,
            "{line:?}"
        );
    }
}

/// Task #301's scan-side arm: a closer in command position restarts command
/// position, so the walk reaches depth 0 at the block's **own** closer written
/// right behind a compound command's closer — bash makes the `;` after a
/// compound command optional. Each row is rc 0 in bash 5.3.9 (measured);
/// `master` found no closer at all, because the closer behind a closer sat in
/// argument position. The end-to-end rows are `tests/block_closers.rs`.
#[test]
fn a_compound_closer_restarts_command_position_in_the_walk() {
    for line in [
        b"while :; do if :; then :; fi done".as_slice(),
        b"if :; then { :; } fi".as_slice(),
        b"if :; then ( : ) fi".as_slice(),
        b"for i in a; do if :; then :; fi done".as_slice(),
        b"while :; do case x in a) : ;; esac done".as_slice(),
        b"if :; then while :; do break; done fi".as_slice(),
    ] {
        let t = toks(line);
        assert_eq!(
            closing_keyword_index(&t, line),
            Some(t.len() - 1),
            "the walk ends at the block's own closer: {line:?}"
        );
    }
    // Behind an argument word the closer-shaped word stays data, so the walk
    // never returns to depth 0 (`echo fi done` runs the body, the block stays
    // open — the rc-1 family of `tests/block_closers.rs`).
    let line = b"while :; do echo fi done";
    assert_eq!(closing_keyword_index(&toks(line), line), None);
    // A clause keyword in argument position does not restart command position
    // either: the `fi` behind the argument-`else` stays data (task #301).
    let line = b"if :; then echo a else fi";
    assert_eq!(closing_keyword_index(&toks(line), line), None);
}

/// Task #309's **seed**: the walk carries the closer of the block token 0 opens,
/// so a closer of a foreign kind written in the body is a word and the walk
/// reaches the block's own closer — and a block with no closer of its own kind
/// yields **no** closing keyword at all, so the builder answers with its own
/// missing-closer message instead of truncating the body at the foreign word.
#[test]
fn the_walk_is_seeded_with_the_closer_of_its_own_block() {
    let lines = [
        // Seeded `Done` (a `while`): the body's `fi` is a word, the walk stops
        // at the loop's own `done` (token 12).
        b"while :; do echo a; fi; echo b; done".as_slice(),
        // Seeded `Fi` (an `if`): the body's `done` and `esac` are words, the
        // walk stops at the `fi` (token 11).
        b"if :; then echo a; done; esac; fi".as_slice(),
        // No closer of the block's own kind left: **no** closing keyword, so the
        // builder reports its own missing closer (the rc-1 rows of
        // `tests/cond_blocks.rs::a_mismatched_closer_that_leaves_no_closer_is_a_parse_error`).
        b"while :; do echo a; fi; echo b".as_slice(),
        // The pair behind an **argument** word never nests (the place rule of
        // task #236), so the pairing never sees the inner loop and the walk
        // stops at its `done` (token 11) — the accepted divergence
        // `tests/cond_blocks.rs::block_stage_in_a_loop_body_ends_the_loop` pins
        // as rc 1 (bash runs the stage, rc 0).
        b"while :; do echo a&&while :; do break; done; done".as_slice(),
    ];
    let found = lines.map(|line| closing_keyword_index(&toks(line), line));
    assert_eq!(found, [Some(12), Some(11), None, Some(11)]);
    // A `case` block seeded with `esac` (what [`case_block`] passes) still
    // terminates on a complete `case` with an empty clause list: the seed
    // matches its own closer, so the pairing pops it — `case x in done)`
    // (a `done` of a pattern list) is pinned one test above.
    let line = b"case x in esac";
    assert_eq!(
        super::list_closing_keyword_index(&toks(line), line, Closer::Esac, super::List::Case),
        Some(3),
        "{line:?}"
    );
}

/// The closer walk's separator rows are gated on the token's raw span (task
/// #336, [`crate::parse::Token::separator_data`]): a quoted `";"` leaves the place an ordinary
/// word leaves, so it restarts no command position and the closer written behind
/// it is a **word**, never the block's closer. Every row stands next to its
/// unquoted twin, so an ungated walk is dead.
#[test]
fn a_quoted_semicolon_word_closes_no_block() {
    let idx = |line: &[u8]| closing_keyword_index(&toks(line), line);
    // The real `;` puts the closer in command position; the quoted word does not.
    assert_eq!(idx(b"while false; do echo a; done"), Some(7));
    assert_eq!(
        idx(b"while false; do echo a \";\" done"),
        None,
        "the `done` is an argument of `echo`"
    );
    assert_eq!(idx(b"while false; do echo a \";\" ; done"), Some(8));
    assert_eq!(idx(b"while false; do echo a ';' done"), None);
    assert_eq!(idx(b"while false; do echo a \\; done"), None);
    // The same for the `fi` and the `esac` closers.
    assert_eq!(idx(b"if :; then echo a; fi"), Some(7));
    assert_eq!(idx(b"if :; then echo a \";\" fi"), None);
    assert_eq!(idx(b"if :; then echo a \";\" ; fi"), Some(8));
    assert_eq!(idx(b"case x in a) echo a;; esac"), Some(9));
    assert_eq!(idx(b"case x in a) echo a \";\" esac"), None);
    // A `for` list keeps its own place rule: the quoted word is a list word, so
    // the list still ends at its `do` and the block closes normally.
    assert_eq!(idx(b"for i in \";\" do echo x; done"), Some(8));
    assert_eq!(idx(b"for i in 1 do echo x; done"), Some(8));
}

/// The walk's **here-doc structure** gate (task #432): an operator's delimiter
/// word, and a word whose span starts inside a terminator line, never score — the
/// same set `brace_expand/protect` protects (task #266). The `{ …; }` group is
/// written at, because it and the definer are the only builders whose closer is a
/// brace word: a brace-shaped delimiter word (`cat << }`, `cat << {`) is not the
/// closer, and neither is a brace-shaped word on a terminator line — the `<<-`
/// row's terminator is tab-indented (the tabs rule). The control is an ordinary
/// brace **argument** word, which still scores: the gate covers structure
/// positions only, and the argument-brace accepted divergence
/// `tests/group.rs::literal_brace_argument_in_a_block_body` keeps its answer.
#[test]
fn a_heredoc_structure_word_never_scores() {
    let idx = |line: &[u8]| closing_keyword_index(&toks(line), line);
    // The delimiter word, both directions. Without the gate the `}` row closes at
    // the delimiter word (token 3) and the `{` row never returns to depth 0.
    assert_eq!(idx(b"{ cat << }; }"), Some(5));
    assert_eq!(idx(b"{ cat << {; }"), Some(5));
    // The terminator line's word, both directions: the body region is blanked and
    // the terminator line's words are not (task #266), so this is where the walk
    // used to be blind. Ungated, the `}` row closes at token 4 and the `{` row
    // never returns.
    assert_eq!(idx(b"{ cat <<}\n}\n}"), Some(6));
    assert_eq!(idx(b"{ cat <<{\n{\n}\n}"), Some(6));
    // The `<<-` form: its terminator line is indented with a tab, and the gate
    // reads the span start inside the terminator span, tabs included.
    assert_eq!(idx(b"{ cat <<-\t}\ny\n\t}\n}"), Some(7));
    // The control: an ordinary brace argument word still scores, so the group
    // closes at it.
    assert_eq!(idx(b"{ echo }; }"), Some(2));
}

/// Task #530 (bug #474): the cond-pair rows read **both** neighbours through the ONE
/// span gate [`crate::parse::Token::span_word`], so a quote-protected `|`/`&`
/// **word** standing next to a real operator token is word data and never a pair half
/// — `echo "|" | cat` is `echo`'s argument through a pipeline, not a split `||` pair.
/// Both neighbour sides are pinned in both directions (a gate that reads only one
/// neighbour, or only the text, is a mutant); the word's **own** quotedness is the
/// walk's `separator_data` row, so `glue_at`'s answer for a quoted word is still what
/// its unquoted neighbour earns. The both-unquoted twins keep their answers.
#[test]
fn a_quoted_operator_word_is_no_pair_half() {
    // The quoted word first, the real operator token behind it: the real token's
    // predecessor row is gated (the pair today), the quoted word's own answer is its
    // neighbour's (unquoted) text plus span.
    let t = toks(b"echo \"|\" | cat");
    assert_eq!(
        glue_at(&t, nth(&t, b"|", 1), b"|"),
        Glue::None,
        "the real `|`: its predecessor is a quoted word"
    );
    assert_eq!(glue_at(&t, nth(&t, b"|", 0), b"|"), Glue::Head);
    // The mirror: the real token first, the quoted word behind it — the successor row
    // is gated the same way.
    let t = toks(b"echo | \"|\" cat");
    assert_eq!(glue_at(&t, nth(&t, b"|", 0), b"|"), Glue::None);
    assert_eq!(glue_at(&t, nth(&t, b"|", 1), b"|"), Glue::Tail);
    // The `&` family, both directions.
    let t = toks(b"echo \"&\" & cat");
    assert_eq!(glue_at(&t, nth(&t, b"&", 1), b"&"), Glue::None);
    assert_eq!(glue_at(&t, nth(&t, b"&", 0), b"&"), Glue::Head);
    let t = toks(b"echo & \"&\" cat");
    assert_eq!(glue_at(&t, nth(&t, b"&", 0), b"&"), Glue::None);
    assert_eq!(glue_at(&t, nth(&t, b"&", 1), b"&"), Glue::Tail);
    // Two real operator tokens: a pair, unchanged (the gate passes on both halves).
    let t = toks(b"echo a & & if :");
    assert_eq!(glue_at(&t, nth(&t, b"&", 0), b"&"), Glue::Head);
    assert_eq!(glue_at(&t, nth(&t, b"&", 1), b"&"), Glue::Tail);
    // Two quoted words: no pair half at all.
    let t = toks(b"echo \"|\" \"|\"");
    assert_eq!(glue_at(&t, nth(&t, b"|", 0), b"|"), Glue::None);
    assert_eq!(glue_at(&t, nth(&t, b"|", 1), b"|"), Glue::None);
}

/// An operator word at index **0** gets no glue: the `i == 0` guard is what
/// keeps [`glue_at`]'s `tokens[i - 1]` from reading back off the list, and every
/// walk above is seeded past zero — so a walk started at 0 measures the guard.
#[test]
fn an_operator_word_at_index_zero_gets_no_glue() {
    let line = b"| x";
    let t = toks(line);
    assert_eq!(
        walk_until(&t, line, None, List::Plain, 0, &|i, _, _| i == 0),
        Some(0)
    );
}

/// The covered side of [`structure_flags`], pinned as the exact list on both
/// sides of the boundary: the terminator line's own word sits **inside** its
/// span, and the word that starts exactly at the span's end sits out — that
/// equality case is the `<` of `covered`.
#[test]
fn a_word_at_the_span_end_is_not_covered() {
    let line = b"cat <<E\nE\ncat x";
    let tk = toks(line);
    let got = structure_flags(line, &tk);
    assert_eq!(
        got,
        vec![false, true, false, true, false, false, false],
        "tokens {tk:?}"
    );
}
