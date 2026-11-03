#![allow(clippy::unwrap_used)]

use super::walk_until;
use crate::keywords::Closer;
use crate::parse::semi::List;
use crate::parse::token::tokenize_statement;
use crate::parse::word_quoted::span_word;
use alloc::vec::Vec;

/// The tokens of `line`, as the parse layer hands them to a walk.
fn toks(line: &[u8]) -> Vec<crate::parse::Token> {
    tokenize_statement(line).unwrap()
}

/// The walk's own `;;` window test, spelled out for the pins: both arms a
/// separator word, exactly as `parse/case_clause/extract.rs` asks it (task #418).
fn window(t: &[crate::parse::Token], i: usize) -> bool {
    t.get(i).is_some_and(|a| {
        t.get(i + 1)
            .is_some_and(|b| span_word(a, b";") && span_word(b, b";"))
    })
}

/// `start` is the first token the walk reads: the closing search keeps skipping
/// its own opener word (token 0), an arm walk reads its first token — it can
/// itself be an opener. A predicate that names token 0 shows the difference.
#[test]
fn start_is_the_first_token_the_walk_reads() {
    let line = b"case b in b) echo y;; esac;; echo x";
    let t = toks(line);
    let at_zero = &|i, _, _| i == 0;
    assert_eq!(
        walk_until(&t, line, Some(Closer::Esac), List::Case, 0, at_zero),
        Some(0)
    );
    assert_eq!(
        walk_until(&t, line, Some(Closer::Esac), List::Case, 1, at_zero),
        None
    );
}

/// The seed is part of the answer: `Some` seeds the pairing, so the walk starts
/// at depth 1 and never reports its own block's closer as the stop; `None` is the
/// top level, depth 0, where that closer is the first token at the level. Same
/// tokens, same predicate, one token of seed — the depth the caller reads.
#[test]
fn the_seed_is_part_of_the_stop_answer() {
    let line = b"case b in b) echo y;; esac;; echo x";
    let t = toks(line);
    // The nested `case` word is the first token at depth 2 for a seeded walk; a
    // walk seeded with nothing never reaches depth 2.
    let at_two = &|_, depth, _| depth == 2;
    assert_eq!(
        walk_until(&t, line, Some(Closer::Esac), List::Case, 0, at_two),
        Some(0)
    );
    assert_eq!(walk_until(&t, line, None, List::Case, 0, at_two), None);
    // Back at the seed: for the seeded walk that is its `esac` (the nested
    // clause's terminator, `List::Case`'s own rule), for the top-level walk it is
    // the same token, reached from depth 0 by the same pop.
    let at_seed = &|_, depth, seed| depth == seed;
    assert_eq!(
        walk_until(&t, line, Some(Closer::Esac), List::Case, 0, at_seed),
        Some(9)
    );
    assert_eq!(walk_until(&t, line, None, List::Case, 0, at_seed), Some(9));
}

/// The clause walk's stop test is a `;;` window **at the walk's own level**:
/// seeded with the `case` block it is inside, it skips the window the nested
/// clause wrote and answers the arm's own (task #418 — the arm whose text is the
/// whole nested clause). The depth-blind predicate — the same scan with no level
/// test — answers the nested window first: that is the bug this task fixed.
#[test]
fn the_window_test_reads_the_walk_s_own_level() {
    let line = b"case b in b) echo y;; esac;; echo x";
    let t = toks(line);
    let blind = &|i, _, _| window(&t, i);
    let at_seed = &|i, depth, seed| depth == seed && window(&t, i);
    assert_eq!(
        walk_until(&t, line, Some(Closer::Esac), List::Case, 0, blind),
        Some(7),
        "the first window is the nested clause's own — the bug this task fixed"
    );
    assert_eq!(
        walk_until(&t, line, Some(Closer::Esac), List::Case, 0, at_seed),
        Some(10),
        "the arm's own window, past the nested block"
    );
    // A region whose windows all belong to a block it never closes: the walk ends
    // without an answer.
    let region = toks(b"case b in b) echo y");
    let at_seed = &|i, depth, seed| depth == seed && window(&region, i);
    assert_eq!(
        walk_until(&region, line, Some(Closer::Esac), List::Case, 0, at_seed),
        None
    );
}
