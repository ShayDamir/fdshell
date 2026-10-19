#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use crate::scan::{Boundary, ScanState, boundary};
use alloc::vec::Vec;

/// The boundary class of every `&` byte a full scan of `line` visits, in byte
/// order. A `Char` is advanced through with the real scanner's [`ScanState`],
/// so a consumed pair (`&&`) is classified once, at its first byte.
fn classes(line: &[u8]) -> Vec<Boundary> {
    let mut state = ScanState::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let kind = boundary(line, i, &state);
        let is_ampersand = matches!(line.get(i), Some(&b'&'));
        if matches!(kind, Boundary::Char) {
            i = state.advance(line, i);
        } else {
            state.word_active = false;
            i += 1;
        }
        if is_ampersand {
            out.push(kind);
        }
    }
    out
}

/// A `>`/`<` written before the `&` **across spaces** owns it (bash tokenizes
/// `>&` as one operator: `cat > &f` is a redirect, never a split) — the rule
/// that the glued-adjacent shapes cannot express, because there the operator
/// byte is the `&`'s own predecessor ([`glued_before`]).
#[test]
fn a_redirect_operator_across_spaces_owns_the_ampersand() {
    let cases: &[(&[u8], &[Boundary])] = &[
        (b"echo a > &", &[Boundary::Char]),
        (b"echo a < &", &[Boundary::Char]),
        (b"echo a > & f", &[Boundary::Char]),
        (b"echo a < &3", &[Boundary::Char]),
        (b"echo a >\t&;", &[Boundary::Char]),
        (b"cat > &f", &[Boundary::Char]),
    ];
    for (line, want) in cases {
        assert_eq!(&classes(line), want, "{line:?}");
    }
}

/// The ownership rule is bounded by the spaces it skips: a newline between the
/// operator and the `&` ends the scan back, so the `&` on the next line is the
/// async operator (each line is scanned as its own statement text).
#[test]
fn a_newline_between_the_operator_and_the_ampersand_ends_the_ownership() {
    assert_eq!(classes(b"echo a >\n& echo b".as_slice()), [Boundary::Async]);
}

/// Positive controls for the same gate: with nothing redirect-owned before it,
/// a standing-alone `&` **is** the async operator (an always-`true`
/// [`redirect_ahead`] would swallow every background job).
#[test]
fn an_ampersand_with_no_redirect_before_it_is_async() {
    let cases: &[(&[u8], &[Boundary])] = &[
        (b"echo a &", &[Boundary::Async]),
        (b"echo a & f", &[Boundary::Async]),
        (b"echo a && f", &[Boundary::Char, Boundary::Char]),
        (b"echo a & f &", &[Boundary::Async, Boundary::Async]),
        (b"echo a <<<x &", &[Boundary::Async]),
    ];
    for (line, want) in cases {
        assert_eq!(&classes(line), want, "{line:?}");
    }
}
