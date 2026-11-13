#![allow(clippy::unwrap_used)]

use super::*;
use crate::parse::token::tokenize;

/// The byte test [`separator_byte`] is the one owner with the byte as data: the
/// tokenizer emits a `;` **token** for a `;` byte, a newline and `)` alike, so
/// only the raw byte tells the spellings apart (task #355's pair, kept
/// load-bearing through [`semi_byte`], its `b';'` reading). A quoted `";"` word
/// and an escaped `\;` (the pair stays in the word text) are word data and
/// match neither byte — the span gate and the byte arm are each half of that
/// rule, so each is pinned on the spelling the other half cannot see.
#[test]
fn the_byte_test_separates_the_semi_byte_from_the_newline() {
    let last = |line: &[u8]| {
        let tokens = tokenize(line).unwrap();
        let tok = tokens.last().unwrap();
        (
            separator_byte(tok, line, b';'),
            separator_byte(tok, line, b'\n'),
            semi_byte(tok, line),
        )
    };
    // (the `;` byte, the newline byte, the `b';'` reading) per row.
    assert_eq!(last(b"echo a ;"), (true, false, true));
    assert_eq!(last(b"echo a\n"), (false, true, false));
    assert_eq!(last(b"echo a \";\""), (false, false, false));
    assert_eq!(last(b"echo a \\;"), (false, false, false));
    assert_eq!(last(b"echo a )"), (false, false, false));
}

/// The gap — at most one real `;` byte, then any run of newlines — is **one
/// rule read from both ends**: [`gap_run`] takes it at the start of the slice
/// (the arithmetic `for` tail) and [`gap_run_at_end`] at the end (the word-form
/// `for` list, task #559). The directions are not mirrors: what the other
/// direction's run starts with is list data for this one (a `;` behind a
/// newline, a second `;` behind a `;`), and the quote/escape spellings stay out
/// of both runs. A comment row needs no arm: the tokenizer drops it with its
/// own newline, so the row **before** it is what stands in the gap. Each cell
/// is the byte test's kill: swap the byte parameters and the `0`/asymmetric
/// rows flip; drop the byte arm and the escaped row joins the run; drop the
/// span gate and the quoted row does.
#[test]
fn the_gap_is_one_rule_read_from_both_ends() {
    let runs = |line: &[u8]| {
        let tokens = tokenize(line).unwrap();
        (gap_run(&tokens, line), gap_run_at_end(&tokens, line))
    };
    // (line, run from the start, run from the end) — both measured. The `;`
    // slot of each direction is optional, so a pure newline run is the gap in
    // both; what makes the directions **not** mirrors is the order the other
    // run starts with — a newline before the `;`, a second `;`, the quote /
    // escape spellings the byte test keeps out — and every such row is a cell
    // only one of the two arms can answer.
    for (line, (front, back)) in [
        (b";".as_slice(), (1, 1)),
        (b"\n".as_slice(), (1, 1)),
        (b";\n".as_slice(), (2, 2)),
        (b";\n\n".as_slice(), (3, 3)),
        (b"\n\n".as_slice(), (2, 2)),
        (b"; ;".as_slice(), (1, 1)),
        (b"\n;".as_slice(), (1, 1)),
        (b";\n;".as_slice(), (2, 1)),
        (b"; ;\n".as_slice(), (1, 2)),
        (b"\";\"\n".as_slice(), (0, 1)),
        (b"\\;\n".as_slice(), (0, 1)),
        (b"\\; ;".as_slice(), (0, 1)),
        (b";\n# c\n".as_slice(), (2, 2)),
        (b";\n# c\nx".as_slice(), (2, 0)),
        (b"x".as_slice(), (0, 0)),
        (b"".as_slice(), (0, 0)),
    ] {
        assert_eq!(runs(line), (front, back), "{line:?}");
    }
}
