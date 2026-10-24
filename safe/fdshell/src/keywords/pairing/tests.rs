#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

/// `while :; do if :; then :; fi done` — the one line the nesting pins read,
/// with each word's own end offset in it.
const NESTED: &[u8] = b"while :; do if :; then :; fi done";

/// A seeded scan (a block span): its own closer is the innermost open block, an
/// opener pushes, and the matching closer pops.
#[test]
fn an_opener_pushes_and_its_own_closer_pops() {
    let mut p = Pairing::new();
    p.open(Closer::Done);
    assert_eq!(p.top(), Some(Closer::Done));
    assert!(p.is_open());
    // A nested `if` pushes the kind it will require.
    assert_eq!(p.apply(b"if", NESTED, 14), 1);
    assert_eq!(p.top(), Some(Closer::Fi));
    // Its own closer pops it, and the outer `done` is next in line.
    assert_eq!(p.apply(b"fi", NESTED, 28), -1);
    assert_eq!(p.top(), Some(Closer::Done));
    assert_eq!(p.apply(b"done", NESTED, 33), -1);
    assert!(!p.is_open());
}

/// The rule of task #309: a closer of **another kind** scores `0` and leaves the
/// stack alone — the word is data, so the loop body keeps its tail (the
/// type-blind depth dropped the body's `break` and the body never returned).
#[test]
fn a_mismatched_closer_is_word_data() {
    let mut p = Pairing::new();
    p.open(Closer::Done);
    assert_eq!(p.apply(b"fi", b"while :; do echo a; fi", 22), 0);
    assert_eq!(p.top(), Some(Closer::Done));
    assert!(p.is_open());
    // `esac` mismatches a `while` too, and so does the group brace.
    assert_eq!(p.apply(b"esac", b"while :; do echo a; esac", 24), 0);
    assert_eq!(p.apply(b"}", b"while :; do echo a; }", 21), 0);
    assert_eq!(p.top(), Some(Closer::Done));
    // The right closer still closes.
    assert_eq!(p.apply(b"done", b"while :; do echo a; done", 24), -1);
    assert!(!p.is_open());
}

/// Nested blocks pair innermost-first: a `done`/`esac` written while an `if` is
/// open mismatches and scores nothing, the `fi` pops the `if`, and only then does
/// the `done` pop the `while`.
#[test]
fn nesting_pairs_innermost_first() {
    let mut p = Pairing::new();
    p.open(Closer::Done);
    assert_eq!(p.apply(b"if", NESTED, 14), 1);
    assert_eq!(p.apply(b"done", NESTED, 33), 0, "the open `if` wants `fi`");
    assert_eq!(p.apply(b"esac", NESTED, 33), 0);
    assert_eq!(p.apply(b"fi", NESTED, 28), -1);
    assert_eq!(p.top(), Some(Closer::Done));
    assert_eq!(p.apply(b"done", NESTED, 33), -1);
    assert!(!p.is_open());
}

/// Top level (nothing open): an opener pushes, and a closer has **no block to
/// mismatch against**, so it keeps the `-1` the top-level scans clamp at `0` and
/// pops nothing. This is the stray-closer clamp the cond-boundary and gate scans
/// read.
#[test]
fn at_top_level_a_closer_pops_nothing() {
    let mut p = Pairing::new();
    assert_eq!(p.top(), None);
    assert_eq!(p.apply(b"fi", b"fi; echo x", 2), -1);
    assert!(!p.is_open());
    assert_eq!(p.apply(b"done", b"done", 4), -1);
    assert!(!p.is_open());
    // An opener opens a block from the top level.
    assert_eq!(p.apply(b"case", b"case x in esac", 4), 1);
    assert_eq!(p.top(), Some(Closer::Esac));
    assert_eq!(p.apply(b"esac", b"case x in esac", 14), -1);
    assert!(!p.is_open());
}

/// A word that is no keyword scores `0` and touches no stack: an argument, a
/// keyword prefix, a quoted keyword word, and the POSIX builtin `wait`.
#[test]
fn a_non_keyword_touches_no_stack() {
    let mut p = Pairing::new();
    p.open(Closer::Done);
    let rows: [(&[u8], &[u8], usize); 4] = [
        (b"echo", b"echo if", 4),
        (b"ifconfig", b"ifconfig", 8),
        (b"\"if\"", b"echo \"if\"", 9),
        (b"wait", b"wait 123", 4),
    ];
    for (word, line, end) in rows {
        assert_eq!(p.apply(word, line, end), 0, "{word:?}");
        assert_eq!(p.top(), Some(Closer::Done), "{word:?}");
    }
    // The event-`wait` block opener is a keyword: it pushes.
    assert_eq!(p.apply(b"wait", b"wait\n readable %rd) x ;;", 4), 1);
    assert_eq!(p.top(), Some(Closer::Done));
}
