//! Unit pins for the glued cond-pair payload ([`pair_payload`]): the keyword
//! candidate the token- and run-layer scorers must read from a glued word, and
//! every shape that carries none (no pair, an empty payload, an
//! operator-shaped payload, the single-byte pipeline `|`).

use super::pair_payload;

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
        assert_eq!(pair_payload(word), Some(payload), "{word:?} carries a pair");
    }
}

/// No pair: a pipeline `|`, a single `&` (background/redirect bytes) and a
/// plain word are all `None` — the single-byte `|` is #292's divergence and
/// never a pair.
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
        assert_eq!(pair_payload(word), None, "{word:?} carries no pair");
    }
}

/// An empty payload is no payload: the pair ends the word, or only
/// whitespace follows it.
#[test]
fn empty_payload_is_none() {
    for word in [b"a||".as_slice(), b"a&&", b"a||  ", b"a && "] {
        assert_eq!(pair_payload(word), None, "{word:?} has no payload");
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
            pair_payload(word),
            None,
            "{word:?} payload is operator bytes"
        );
    }
}
