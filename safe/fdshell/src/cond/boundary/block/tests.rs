//! Unit pins for keyword-block opacity in the cond-list boundary scan: a
//! `&&` / `||` written inside a keyword block's text is never a boundary of
//! the outer cond list, and the shapes that must *not* take the jump
//! (closers, quoted keywords, substitution bodies, keyword-shaped words,
//! group openers, heredoc-carrying spans) keep splitting exactly as before.

use crate::cond::boundary::next_boundary;
use crate::scan::ScanState;

/// `next_boundary` from the start of `line`; `line.len()` means "no split —
/// the text is one cond-list part".
fn boundary_of(line: &[u8]) -> (usize, bool) {
    next_boundary(line, 0, &mut ScanState::new())
}

/// The shapes of the reported bug: the whole block is one part, so the
/// `&&` / `||` reaches the block's own cond list (bash runs it there).
#[test]
fn operator_in_a_block_condition_is_not_a_boundary() {
    for line in [
        b"while [ 1 ] && [ 1 ]; do a; done".as_slice(),
        b"until [ 1 ] || [ 1 ]; do a; done".as_slice(),
        b"if [ 1 ] && [ 0 ]; then t; else f; fi".as_slice(),
        b"for i in a; do a && b; done".as_slice(),
        b"case x in a) false && echo b;; esac".as_slice(),
        b"while if true; then false; fi && true; do a; done".as_slice(),
        b"time while [ 1 ] && [ 1 ]; do a; done".as_slice(),
    ] {
        assert_eq!(boundary_of(line), (line.len(), false), "{:?}", line);
    }
}

/// The scan resumes just past the block's closing keyword, so a *trailing*
/// operator is still found, and a group wrapped around a block keeps its
/// split after the group's own `}` (the group rule and the jump coexist).
#[test]
fn scan_resumes_past_the_closing_keyword() {
    // `… done && echo b` — the jump lands at the space after `done` (22).
    assert_eq!(
        boundary_of(b"while true; do a; done && echo b"),
        (23, false)
    );
    // `… } && echo d` — the split is the group's operator, at 35.
    assert_eq!(
        boundary_of(b"{ while c && c; do b && c; done; } && echo d"),
        (35, false)
    );
}

/// `wait` opens an event-case block only with a pattern keyword behind it;
/// the POSIX builtin keeps its `&&` as a cond-list operator.
#[test]
fn wait_block_is_opaque_and_posix_wait_splits() {
    let block = b"wait\nreadable %rd) false && echo b;;\ndone";
    assert_eq!(boundary_of(block.as_slice()), (block.len(), false));
    // `wait $! && echo x` — the builtin: split at the `&&` (8).
    assert_eq!(boundary_of(b"wait $! && echo x"), (8, false));
    // A same-line `;` is the builtin too: the `&&` splits (after `done`, at 30).
    assert_eq!(
        boundary_of(b"wait; readable) echo r;; done && echo x"),
        (30, false)
    );
}

/// An unclosed opener yields the whole rest of the text as one part: the
/// parse layer reports the missing closer, as it does today.
#[test]
fn unclosed_opener_is_one_part() {
    let line = b"while [ 1 ] && [ 1 ]";
    assert_eq!(boundary_of(line.as_slice()), (line.len(), false));
    let line = b"if [ 1 ] && [ 0 ]; then t";
    assert_eq!(boundary_of(line.as_slice()), (line.len(), false));
}

/// A span that carries a `<<` operator does not take the jump: the block's
/// condition bodies travel inside its text, and the `&&` written in a body
/// line must keep splitting the part (see `tests/heredoc.rs`).
#[test]
fn heredoc_carrying_span_keeps_the_split() {
    // `while cat <<X; do a && b; X` + newline + `done` — split at the `&&` (20).
    assert_eq!(
        boundary_of(b"while cat <<X; do a && b; X\ndone"),
        (20, false)
    );
    // `if cat <<X; then` + body with `&&` + delimiter + `fi` — the body `&&` (22).
    assert_eq!(
        boundary_of(b"if cat <<X; then\nbody && x\nX\nfi"),
        (22, false)
    );
}

/// Kill-shapes for the gate: only an *opening* keyword word at a word start
/// takes the jump. Closers, quoted and substituted keywords, keyword-shaped
/// words and group openers must keep splitting.
#[test]
fn gate_fires_only_for_an_opening_keyword_word() {
    // Closers score -1.
    assert_eq!(boundary_of(b"done && echo x"), (5, false));
    assert_eq!(boundary_of(b"fi && echo x"), (3, false));
    assert_eq!(boundary_of(b"esac && echo x"), (5, false));
    assert_eq!(boundary_of(b"} && echo b"), (2, false));
    // A quoted keyword word, and a keyword inside a `$( … )` body.
    assert_eq!(boundary_of(b"\"while\" && echo x"), (8, false));
    assert_eq!(
        boundary_of(b"echo \"$(while true; do a; done)\" && echo b"),
        (33, false)
    );
    assert_eq!(
        boundary_of(b"$( while true; do a; done ) && echo b"),
        (28, false)
    );
    // Keyword prefixes are words, not keywords.
    assert_eq!(boundary_of(b"whileish && echo x"), (9, false));
    assert_eq!(boundary_of(b"iffy && echo x"), (5, false));
    // A group opener is a cond-list part: its own rule splits after the brace.
    assert_eq!(boundary_of(b"echo { && echo b"), (7, false));
    assert_eq!(boundary_of(b"{ echo a; } && { echo b; }"), (12, false));
}
