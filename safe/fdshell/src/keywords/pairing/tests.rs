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

/// The **depth** a walk carries — the number of open blocks, which is the
/// window test's level (task #418): the seed is the block the caller is already
/// inside, an opener pushes, the matching closer pops back to the seed, and a
/// mismatched closer scores nothing so the depth stays where the imbalance warns
/// it stays. The top level seeds nothing, so depth `0` is where it stops.
#[test]
fn depth_is_the_level_the_walk_reads() {
    let mut p = Pairing::new();
    assert_eq!(p.depth(), 0, "the top level: nothing seeded");
    p.open(Closer::Esac);
    assert_eq!(p.depth(), 1, "the `case` block the walk is inside");
    // A nested `case` pushes its own kind — its `;;` belongs to it.
    assert_eq!(p.apply(b"case", b"case b in b) echo y;; esac", 4), 1);
    assert_eq!(p.depth(), 2);
    // A closer of another kind is word data, so the depth stays.
    assert_eq!(p.apply(b"fi", b"case b in b) echo y;; fi", 22), 0);
    assert_eq!(p.depth(), 2);
    // The nested block's own closer pops it: the walk is back at its level.
    assert_eq!(p.apply(b"esac", b"case b in b) echo y;; esac", 26), -1);
    assert_eq!(p.depth(), 1);
    // The seed is never popped by the walk: it is what "at my own level" means.
    p.open(Closer::Esac);
    assert_eq!(p.depth(), 2);
    assert_eq!(p.apply(b"esac", b"case x in esac", 13), -1);
    assert_eq!(p.depth(), 1);
}

/// Task #436's **decision**: the same stack, read for the report — one rule, no
/// second table, no score moved. Both sides of every arm are pinned (§2.8): a
/// matched closer is `Paired` (the seeded case and the pushed case), a closer of
/// another kind is `Stray`, and so is a closer seen with **nothing open** — the
/// arm the scan reports, at the words its place gate puts in command position.
/// A word that is no keyword and an opener answer `None`. Task #467 brought the
/// **brace** closer into the decision — bash 5.3.9 refuses a lone `}` in command
/// position at rc 2, nothing runs (measured 2026-11-07; the house rc is 1) — so
/// it answers like every other closer on both arms. The walk's recorder
/// ([`Pairing::observe`]) is the half that keeps not recording braces
/// (`comment/tests.rs::the_scan_records_a_top_level_closer`).
#[test]
fn the_decision_reads_the_same_stack() {
    // A matched closer — the seed's kind, and a kind an opener pushed.
    let mut p = Pairing::new();
    p.open(Closer::Fi);
    assert_eq!(p.decision(b"fi", b"echo a; fi", 10), Some(Decision::Paired));
    assert_eq!(p.apply(b"fi", b"echo a; fi", 10), -1);
    p.open(Closer::Done);
    assert_eq!(p.apply(b"if", b"if :; then :; fi", 2), 1);
    assert_eq!(
        p.decision(b"fi", b"if :; then :; fi", 18),
        Some(Decision::Paired)
    );
    // The kind-mismatch arm: word data for the score, `Stray` for the decision.
    let mut p = Pairing::new();
    p.open(Closer::Done);
    assert_eq!(
        p.decision(b"fi", b"while :; do echo a; fi", 22),
        Some(Decision::Stray)
    );
    assert_eq!(
        p.decision(b"esac", b"while :; do echo a; esac", 24),
        Some(Decision::Stray)
    );
    assert_eq!(
        p.apply(b"fi", b"while :; do echo a; fi", 22),
        0,
        "the score stays a score"
    );
    // Nothing open: the arm the scan of task #436 reports.
    let mut p = Pairing::new();
    assert_eq!(p.decision(b"fi", b"echo a; fi", 9), Some(Decision::Stray));
    assert_eq!(
        p.decision(b"done", b"echo a; done", 11),
        Some(Decision::Stray)
    );
    assert_eq!(
        p.decision(b"esac", b"echo a; esac", 12),
        Some(Decision::Stray)
    );
    assert_eq!(
        p.apply(b"fi", b"echo a; fi", 9),
        -1,
        "the clamp every top-level scan reads"
    );
    // Not asked: a word that is no keyword, and an opener.
    assert_eq!(p.decision(b"echo", b"echo if", 4), None);
    assert_eq!(p.decision(b"if", b"if :; then :; fi", 2), None);
    // The brace closer answers on both arms now (task #467): nothing open is
    // `Stray`, the group's own closer is `Paired`.
    assert_eq!(p.decision(b"}", b"echo a; }", 9), Some(Decision::Stray));
    p.open(Closer::Brace);
    assert_eq!(p.decision(b"}", b"{ echo a; }", 10), Some(Decision::Paired));
    assert_eq!(
        p.apply(b"}", b"{ echo a; }", 10),
        -1,
        "the score still pairs it"
    );
}
