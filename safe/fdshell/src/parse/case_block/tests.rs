#![allow(clippy::unwrap_used)]

use super::tokens_to_case;
use crate::mask::MaskBit;
use crate::mask::MaskBit::Unquoted;
use crate::mask::tests::runs;
use crate::parse::token::tokenize_statement;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The `case` word and its mask, as the parser hands them to `case_exec`.
fn word_of(line: &[u8]) -> (ShortCStr, Vec<MaskBit>) {
    let text = stext(line);
    let tokens = tokenize_statement(line).unwrap();
    let block = tokens_to_case(&tokens, &text).unwrap();
    (block.word, block.word_mask)
}

fn sc(text: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(text.to_vec()).unwrap()
}

/// The message of the parse error `tokens_to_case` reports for `line` — empty
/// when the block parses.
fn err_message(line: &[u8]) -> String {
    let text = stext(line);
    let tokens = tokenize_statement(line).unwrap();
    match tokens_to_case(&tokens, &text) {
        Ok(_) => String::new(),
        Err(report) => report.current_context().to_string(),
    }
}

/// The clause count of a block that parses, `None` when it does not.
fn clause_count(line: &[u8]) -> Option<usize> {
    let text = stext(line);
    let tokens = tokenize_statement(line).ok()?;
    tokens_to_case(&tokens, &text).map(|b| b.clauses.len()).ok()
}

/// The `case` word is substituted with its own mask (task #269): a quoted
/// word's bytes are all quoted.
#[test]
fn word_mask_of_a_quoted_word() {
    assert_eq!(
        word_of(b"case \"a\\ b\" in *) echo m;; esac"),
        (sc(b"a\\ b"), runs(&[(true, 4)]))
    );
}

/// An unquoted word carries an all-unquoted mask, escape pairs included (the
/// pairs stay in the token text, so `a\ b` is a 4-byte word: `a`, `\`, ` `, `b`).
#[test]
fn word_mask_of_an_unquoted_word() {
    assert_eq!(
        word_of(b"case a\\ b in *) echo m;; esac"),
        (sc(b"a\\ b"), vec![Unquoted; 4])
    );
}

/// A multi-token join inserts one `false` bit per inserted space, and every
/// token keeps its own bits: the mask stays parallel to the joined text.
#[test]
fn word_mask_of_a_joined_word() {
    let (case_word, mask) = word_of(b"case a \"b c\" d in *) echo m;; esac");
    assert_eq!(case_word, sc(b"a b c d"));
    assert_eq!(mask, runs(&[(false, 2), (true, 3), (false, 2)]));
}

/// The word region is `;`-trimmed, and a word that **is** empty keeps an empty
/// mask — an empty quote pair contributes no byte. The unquoted spellings that
/// used to sit here (`case in …`, `case ; ; in …`) have no word at all and are
/// errors now (task #415, pinned by [`the_empty_word_is_the_word_error]).
#[test]
fn word_mask_of_a_quoted_empty_word_is_empty() {
    let (case_word, mask) = word_of(b"case \"\" in *) echo m;; esac");
    assert_eq!(case_word, sc(b""));
    assert!(mask.is_empty());
    // A quoted `";"` is a word, so its byte is in the joined text — the
    // separator trim never touches it (task #336).
    let (case_word, mask) = word_of(b"case \";\" in *) echo m;; esac");
    assert_eq!(case_word, sc(b";"));
    assert_eq!(mask, runs(&[(true, 1)]));
}

/// Every spelling of an empty `case` word is a parse error (task #415): POSIX
/// #2.9.1's `case_clause` production requires a `word` between `case` and `in`,
/// so bash 5.3.9 rejects each of these with rc 2 (measured) and fdshell answers
/// with its own message at rc 1. A `;`, the newline that tokenizes as one, and a
/// comment-only word all trim to nothing (task #299's rule: the test is on the
/// trimmed slice). The quoted spellings hold a **token**, so they stay words and
/// stay green — bash accepts them at rc 0.
#[test]
fn the_empty_word_is_the_word_error() {
    for line in [
        b"case; in esac".as_slice(),
        b"case ; in esac",
        b"case ; ; in *) echo m;; esac",
        b"case in esac",
        b"case in *) echo m;; esac",
        b"case\nin esac",
        b"case\nin a) echo A;; esac",
        b"case # c\nin esac",
    ] {
        assert_eq!(err_message(line), "case: empty word", "{line:?}");
    }
    for line in [
        b"case \"\" in esac".as_slice(),
        b"case \";\" in esac",
        b"case \"\" in *) echo m;; esac",
        b"case x in esac",
    ] {
        assert_eq!(err_message(line), "", "{line:?}");
    }
}

/// The `case` word is a `;`-trimmed slice, so a separator between the word and
/// `in` is not part of the clause-list guard's scan.
#[test]
fn a_separator_before_in_is_trimmed_out_of_the_word_slot() {
    for line in [
        b"case x; in esac".as_slice(),
        b"case x ; in esac",
        b"case x\n; in esac",
    ] {
        assert_eq!(clause_count(line), Some(0), "{line:?}");
    }
}

/// A real `;` **byte** in the empty clause list is the grammar violation, while
/// the newline that tokenizes as the same `;` **token** is not (task #415):
/// POSIX #2.9.1's production there is a `newline_list`, and bash 5.3.9 rejects
/// the `;` spellings with rc 2 (measured) while `case x in esac` and its newline
/// spelling run at rc 0. Only the **empty** list is guarded — a `;` in front of
/// a clause keeps fdshell's documented leniency (task #300, pinned
/// end-to-end in `tests/case.rs`).
#[test]
fn a_semicolon_byte_in_an_empty_clause_list_is_the_clause_error() {
    for line in [
        b"case x in; esac".as_slice(),
        b"case x in ; esac",
        b"case x in;; esac",
        b"case x in ;;; esac",
        b"case x in\n; esac",
        b"case x in ;\nesac",
        b"case x in # c\n; esac",
    ] {
        assert_eq!(
            err_message(line),
            "case: ';' before an empty clause list",
            "{line:?}"
        );
    }
    // The two rows that hold the byte test from the other side: an empty slot
    // with no token at all, and one carried only by newline tokens.
    assert_eq!(clause_count(b"case x in esac"), Some(0));
    assert_eq!(clause_count(b"case x in\nesac"), Some(0));
}

/// The `in` search is quote-gated (task #247): a quoted `in` is word data, so
/// the block reports its missing `in` (bash: a syntax error, rc 2). The word
/// rules of the clause list are untouched: a complete empty block and a
/// closer-shaped **pattern** stay as they were (task #273).
#[test]
fn in_word_is_a_keyword_only_when_unquoted() {
    let missing_in = |line: &[u8]| {
        let text = stext(line);
        let tokens = tokenize_statement(line).unwrap();
        matches!(
            tokens_to_case(&tokens, &text).map(|_| ()),
            Err(report)
                if matches!(
                    report.current_context(),
                    crate::error::parse::ParseError::CaseMissingIn
                )
        )
    };
    assert_eq!(clause_count(b"case x in esac"), Some(0));
    assert_eq!(clause_count(b"case x in done) echo D;; esac"), Some(1));
    for line in [
        b"case x 'in' a) echo A;; esac".as_slice(),
        b"case x \"in\" a) echo A;; esac",
        b"case x ''in a) echo A;; esac",
    ] {
        assert!(missing_in(line), "{line:?} must report the missing `in`");
    }
}

/// A clause closed directly by `esac` is a clause with an **empty body** (task
/// #300, bash rc 0), and the clause list keeps counting after it. The complete
/// empty block stays zero clauses (the shape task #273 pins above).
#[test]
fn a_clause_closed_by_esac_is_a_clause_with_an_empty_body() {
    let block = |line: &[u8]| {
        let text = stext(line);
        let tokens = tokenize_statement(line).unwrap();
        tokens_to_case(&tokens, &text)
            .unwrap_or_else(|e| panic!("{line:?}: {}", e.current_context()))
    };
    let b = block(b"case x in a) esac");
    assert_eq!(b.clauses.len(), 1);
    let first = b.clauses.first().unwrap();
    assert!(first.body.data.is_empty());
    assert!(first.patterns.first().unwrap().text.eq_bytes(b"a"));

    let b = block(b"case x in a) echo one;; b) esac");
    assert_eq!(b.clauses.len(), 2);
    assert!(b.clauses.first().unwrap().body.data.eq_bytes(b"echo one"));
    assert!(b.clauses.get(1).unwrap().body.data.is_empty());

    assert_eq!(block(b"case x in esac").clauses.len(), 0);
}

/// Task **#470**: a word-opening `(` and the bare `)` are the control operators
/// POSIX 2.9.4.3 puts in front of the `case WORD` (bash 5.3.9 rejects every
/// unquoted spelling at rc 2, measured), and the gate is the shared
/// `word_quoted::slot_rejects`, asked of every token of the **trimmed** slot,
/// after the empty-word guard and before the join and the clause walk (bash
/// reports the earliest offending token, the order the `for` site documents). The
/// slot's owned set is only the `;` `trim_semi` owns — every other row of the ONE
/// set rejects with it (the `&`/`|`/`&&`/`||` rows, R2). The quoted, escaped and
/// mid-word spellings stay accepted: the mask bit, the backslash and the
/// mid-word position are the span gate's family (LESSONS: "A separator-shaped
/// word needs the span gate"), and the mid-slot `;` is the owned set's own row —
/// the row that keeps the closure's comparison from being decorative.
#[test]
fn the_word_slot_rejects_the_operators_bash_rejects() {
    let msg = "case: '(' and ')' are control operators here (quote them)";
    for line in [
        b"case (abc) in x) echo m;; esac".as_slice(),
        b"case ( ) in x) echo m;; esac".as_slice(),
        b"case a ) b in x) echo m;; esac".as_slice(),
        b"case a & b in x) echo m;; esac".as_slice(),
        b"case a | b in x) echo m;; esac".as_slice(),
        b"case a && b in x) echo m;; esac".as_slice(),
        b"case a || b in x) echo m;; esac".as_slice(),
    ] {
        assert_eq!(err_message(line), msg, "{line:?}: bash rc 2");
    }
    for line in [
        b"case x ; in esac".as_slice(),
        b"case x ; y in esac".as_slice(),
        b"case \"a(b\" in esac".as_slice(),
        b"case \\(b in esac".as_slice(),
        b"case a(b in esac".as_slice(),
    ] {
        assert_eq!(err_message(line), "", "{line:?}");
    }
}
