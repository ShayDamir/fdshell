//! Unit pins for the glued-`)` closer payload ([`closer_payload`]): the closer
//! candidate behind a `case` pattern word's `)` (task #421), and every shape that
//! carries none — no `)` byte, a `)` that ends the word, an empty payload, an
//! arithmetic region word. The **gates** (place, kind, the stray-closer clamp)
//! stay the scorer's, so a payload they reject is still returned here; the
//! one-score-per-word rule is the scorer's too
//! (`comment/tests.rs::the_word_start_row_owns_a_word_it_recognizes`).

use super::closer_payload;

/// A `)` inside the word, with a closer behind it, carries the payload: the
/// pattern word's `)` is the glue byte and the closer is the block's closing
/// keyword — the redirection operand behind it included (task #413), quotes
/// included (they belong to the operand).
#[test]
fn closer_behind_the_paren_is_the_payload() {
    for (word, payload) in [
        (b"a)esac".as_slice(), b"esac".as_slice()),
        (b"b)esac>f", b"esac>f"),
        (b"b)esac>f\"f\"", b"esac>f\"f\""),
        (b"a)fi", b"fi"),
        (b"a)done", b"done"),
        (b"a)esac echo y", b"esac echo y"),
    ] {
        assert_eq!(closer_payload(word), Some(payload), "{word:?}");
    }
}

/// A **quoted** payload is returned as the raw bytes it is: the quote gate lives
/// in the recognition table the scorer calls with the payload, not here (task
/// #413 — the quotes belong to the operand, never to the keyword). A single-quoted
/// word never reaches the scorer at all, so it is not this predicate's row.
#[test]
fn a_quoted_payload_is_returned_raw() {
    assert_eq!(closer_payload(b"a)\"esac\""), Some(b"\"esac\"".as_slice()));
}

/// No payload: the word carries no `)`, or its `)` ends the word (the plain glued
/// pattern `a)`, whose own rows are the word-start rule's), or nothing meaningful
/// sits behind it.
#[test]
fn no_paren_or_an_empty_payload_is_none() {
    for word in [
        b"echo".as_slice(),
        b"a)",
        b"esac)",
        b"done)",
        b"fi)",
        b"",
        b"(a*",
    ] {
        assert_eq!(closer_payload(word), None, "{word:?}");
    }
}

/// An arithmetic word's `)` are **region bytes**, never the clause close (task
/// #396's carve-out): the region written as a word of its own, and the `for` head
/// glued to it.
#[test]
fn an_arithmetic_region_carries_no_closer() {
    for word in [
        b"((x=0;x<2;x++))esac".as_slice(),
        b"for((x=0;x<2;x++))esac",
        b"((1))esac",
        b"for((i))esac",
    ] {
        assert_eq!(closer_payload(word), None, "{word:?}");
    }
}
