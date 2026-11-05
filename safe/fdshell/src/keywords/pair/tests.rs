//! Unit pins for the glued cond-pair payload ([`pair_payload`]): the keyword
//! candidate the token- and run-layer scorers must read from a glued word, and
//! every shape that carries none (no pair, an empty payload, an
//! operator-shaped payload, the single-byte pipeline `|`).

use super::pair_payload;
use crate::mask::MaskBit;

/// A pair in the word's interior yields the payload behind its operator
/// bytes, whitespace-skipped (task #236's S2: `do echo a||if :; …; fi` must
/// nest the `if` its payload opens, or the `fi` closes the enclosing block).
#[test]
fn pair_payload_is_the_word_behind_the_pair() {
    for (word, payload) in [
        (b"a||if".as_slice(), b"if".as_slice()),
        (b"a&&if", b"if"),
        (b"a && fi", b"fi"),
        (b"a||fi", b"fi"),
        (b"&&if", b"if"),
        (b"x||if :", b"if :"),
        (b"a&&time if", b"time if"),
    ] {
        assert_eq!(
            pair_payload(word, &[]),
            Some(payload),
            "{word:?} carries a pair"
        );
    }
}

/// No pair: a pipeline `|`, a single `&` (background/redirect bytes) and a
/// plain word are all `None` — a single-byte `|` is never a pair (the word
/// behind it is scored by the stage arm, task #292).
#[test]
fn pipeline_and_background_bytes_are_no_pair() {
    for word in [
        b"a|if".as_slice(),
        b"a|b",
        b"|if",
        b"a&b",
        b"&if",
        b"a>b",
        b"echo",
        b"",
    ] {
        assert_eq!(pair_payload(word, &[]), None, "{word:?} carries no pair");
    }
}

/// An empty payload is no payload: the pair ends the word, or only
/// whitespace follows it.
#[test]
fn empty_payload_is_none() {
    for word in [b"a||".as_slice(), b"a&&", b"a||  ", b"a && "] {
        assert_eq!(pair_payload(word, &[]), None, "{word:?} has no payload");
    }
}

/// An operator-shaped payload is a redirect form or a longer operator run,
/// never a keyword candidate (`a|||b`, `a||&b`, `a&&&&b`).
#[test]
fn operator_shaped_payload_is_none() {
    for word in [
        b"a|||b".as_slice(),
        b"a||&b",
        b"a&&&&b",
        b"a||||if",
        b"a&& &b",
    ] {
        assert_eq!(
            pair_payload(word, &[]),
            None,
            "{word:?} payload is operator bytes"
        );
    }
}

/// Task #530 (bug #474): the pair row answers through the word's own **quote mask**,
/// the ONE predicate — in `a"&&"if` the pair's own bytes are quoted, so POSIX §2.9.1
/// makes the word one argument and it carries no pair at all (`echo a"&&"if :` prints
/// `a&&if :`, and the word behind it is never a command). The mirror shape `a&&"if"` —
/// the operator unquoted, the **payload** quoted — keeps its payload (task #413's pins in
/// `keywords/kind/tests.rs`), so the gate reads the operator's bytes, not the payload's.
/// Both directions of the mask are pinned; the byte-level family's empty mask pads to
/// *unquoted* and keeps the text-only rule (those callers are byte-identical, and the byte
/// splitters break words on raw operator bytes, so they never see a quoted pair).
#[test]
fn a_quoted_operator_byte_carries_no_pair() {
    let (q, u) = (MaskBit::QuotedStart, MaskBit::Unquoted);
    for (word, mask, want) in [
        // The operator bytes quoted: one word, no pair.
        (b"a&&if".as_slice(), &[u, q, q, u, u][..], None),
        (b"a||if", &[u, q, q, u, u][..], None),
        // The operator unquoted (the payload quoted): the row still fires.
        (b"a&&if", &[u, u, u, q, q][..], Some(b"if".as_slice())),
        (b"a||if", &[u, u, u, q, q][..], Some(b"if".as_slice())),
        // Both unquoted.
        (b"a&&if", &[u, u, u, u, u][..], Some(b"if".as_slice())),
        // The byte-level family: an empty mask pads to unquoted, text rule unchanged.
        (b"a&&if", &[][..], Some(b"if".as_slice())),
    ] {
        assert_eq!(
            pair_payload(word, mask),
            want,
            "word={word:?} mask={mask:?}"
        );
    }
}
