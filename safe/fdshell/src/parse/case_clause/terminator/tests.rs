#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::{Terminator, kind_at, tokens};
use crate::parse::Token;
use crate::parse::token::tokenize_statement;
use Terminator::{Retest, RunNext, Stop};
use alloc::vec;
use alloc::vec::Vec;

/// The tokens of a whole `case` line. The predicate only ever looks at a token and
/// its two neighbours, so for these pins the whole line is the arm region.
fn toks(line: &[u8]) -> Vec<Token> {
    tokenize_statement(line).unwrap()
}

/// The predicate's answer at every token that **looks like** a `;` word — the
/// tokens the clause walk is asked about, in source order (the walk asks at every
/// index; the other tokens answer `None` at any index, so only these tell). The
/// first `Some` is the window the walk stops at, so a whole expected list says both
/// which kind the family assigns and which candidate wins. The look-alike test
/// covers the tokenizer's newline-as-`;` token and a quoted `";"` word, which
/// task #336's raw-span gate then answers.
fn asked(line: &[u8]) -> Vec<Option<Terminator>> {
    let tokens = toks(line);
    let mut out = Vec::new();
    for i in 0..tokens.len() {
        if tokens
            .get(i)
            .is_some_and(|t| t.0.eq_bytes(b";") || t.0.eq_bytes(b"\";\""))
        {
            out.push(kind_at(&tokens, line, i));
        }
    }
    out
}

/// The three kinds of the family, each glued: `;&`, `;;&`, `;;`.
#[test]
fn the_three_kinds_are_told_apart() {
    assert_eq!(
        asked(b"case x in a) echo A;& b) echo B;; esac"),
        vec![Some(RunNext), Some(Stop), None]
    );
    assert_eq!(
        asked(b"case x in a) echo A;;& b) echo B;; esac"),
        vec![Some(Retest), Some(RunNext), Some(Stop), None]
    );
    assert_eq!(
        asked(b"case x in a) echo A;; b) echo B;; esac"),
        vec![Some(Stop), None, Some(Stop), None]
    );
}

/// Both directions of the glue (LESSONS "a parameter that never changes the answer
/// is an equivalent mutant"): a spaced `&` is not the terminator token, a spaced
/// first `;` is not the glued pair, and the walk's first `Some` is what it acts on.
/// Every one of these rows is the answer it had before task #420 (decision D2).
#[test]
fn a_spaced_pair_terminates_nothing_new() {
    // `; &` — no glue, no terminator at all (the accepted `&` family).
    assert_eq!(asked(b"case x in a) echo a ; & esac"), vec![None]);
    // `; ;&` — the pair is not glued, so the walk stops at the plain `;;` window;
    // the glued reading behind it is never reached.
    assert_eq!(
        asked(b"case x in a) echo X ; ;& b) echo B;; esac"),
        vec![Some(Stop), Some(RunNext), Some(Stop), None]
    );
    // `;; &` — the pair is glued but the `&` is not, so it is the plain `;;`.
    assert_eq!(
        asked(b"case x in a) echo A;; & b) echo B;; esac"),
        vec![Some(Stop), None, Some(Stop), None]
    );
    // `a&;;` — the `&` is body text (the job marker of tasks #354/#355) and the
    // glued pair behind it is the plain stop.
    assert_eq!(
        asked(b"case x in a) echo a&;; b) echo B;; esac"),
        vec![Some(Stop), None, Some(Stop), None]
    );
}

/// The raw-span gate of task #336 reaches the new arms too: a quoted `";"` or
/// `"&"` is word data, glued or not, and a mixed pair answers like the pure
/// unquoted one only where both words are unquoted.
#[test]
fn a_quoted_word_of_the_pair_is_body_data() {
    assert_eq!(
        asked(b"case x in a) echo A \";\" \"&\" b) esac"),
        vec![None]
    );
    // A quoted word with a `&` inside it is ONE word (`;&&`): the run is not a
    // candidate at all, so neither half of the pair is a terminator.
    assert_eq!(asked(b"case x in a) echo A \";\"&\"&\" b) esac"), vec![]);
    // A real `;` glued to a quoted `&`.
    assert_eq!(asked(b"case x in a) echo A ;\"&\" b) esac"), vec![None]);
}

/// Decision D1, as measured: a terminator's head is a **real** `;` byte, so the
/// newline token in front of a glued pair is bash's list separator, not the
/// terminator's head — the row bash accepts (`echo A\n;& b)`, rc 0, `A`,`B`) gets
/// its `;&` from the real bytes, exactly as bash reads it. The plan's D1 wording
/// (no byte test on the new arms) would read the newline as the `;` of a glued
/// `;;&` and answer `A` alone.
#[test]
fn a_newline_token_is_not_the_head_of_a_terminator() {
    assert_eq!(
        asked(b"case x in a) echo A\n;& b) echo B;; esac"),
        vec![None, Some(RunNext), Some(Stop), None]
    );
    // The price, pinned as measured: bash's `;;` + newline + `&` is a syntax error,
    // fdshell answers the plain `;;` and the `&` in the next clause's pattern slot.
    assert_eq!(
        asked(b"case x in a) echo A;;\n& b) echo B;; esac"),
        vec![Some(Stop), Some(Retest), None, Some(Stop), None]
    );
}

/// The resume step past a terminator (the walk's `end + tokens(kind)`, which was
/// the hard-coded `end + 2` until the third kind needed three).
#[test]
fn a_terminator_takes_two_or_three_tokens() {
    assert_eq!(tokens(Stop), 2);
    assert_eq!(tokens(RunNext), 2);
    assert_eq!(tokens(Retest), 3);
}
