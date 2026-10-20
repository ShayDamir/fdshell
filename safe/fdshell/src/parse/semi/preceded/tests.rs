#![allow(clippy::unwrap_used)]
use super::*;
use crate::parse::token::tokenize;

/// `find_preceded_by` reads `needle` through the keyword gate (`keyword_word`,
/// task #247): a quoted `then`/`done` is a command word, so the block finds no
/// keyword — the parse error is the bash-parity outcome (bash rejects the shape
/// with a syntax error).
#[test]
fn preceded_by_semi_needs_an_unquoted_keyword_word() {
    let find = |line: &[u8], start: usize, needle: &[u8]| {
        find_preceded_by(&tokenize(line).unwrap(), start, needle, line, Term::Semi)
    };
    assert_eq!(find(b"if true; then echo a; fi", 1, b"then"), Some(3));
    assert_eq!(find(b"if true; 'then' echo a; fi", 1, b"then"), None);
    assert_eq!(find(b"if true; \"then\" echo a; fi", 1, b"then"), None);
    // The empty quote pair contributes no byte and no mask bit — the span
    // rule catches it (task #321's lesson, at the keyword gate).
    assert_eq!(find(b"if true; ''then echo a; fi", 1, b"then"), None);
    assert_eq!(find(b"while :; do :; done", 1, b"done"), Some(6));
    assert_eq!(find(b"while :; do :; 'done'", 1, b"done"), None);
}

/// The `Term` matrix: a plain `&` is a compound-list terminator, so the clause
/// keyword behind it is found with [`Term::SemiAmp`] (task #354, bash:
/// `if true & then :; fi` is valid). A `for` word list takes no list
/// terminator, so its `do` is searched with [`Term::Semi`] alone and
/// `for i in 1 & do :; done` stays bash's syntax error.
#[test]
fn the_amp_is_a_list_terminator_for_a_compound_list_only() {
    let find = |line: &[u8], start: usize, needle: &[u8], term: Term| {
        find_preceded_by(&tokenize(line).unwrap(), start, needle, line, term)
    };
    assert_eq!(
        find(b"if true & then echo x; fi", 1, b"then", Term::SemiAmp),
        Some(3)
    );
    assert_eq!(
        find(b"if true & then echo x; fi", 1, b"then", Term::Semi),
        None
    );
    assert_eq!(
        find(b"while : & do :; break; done", 1, b"do", Term::SemiAmp),
        Some(3)
    );
    assert_eq!(
        find(
            b"if false; then :; elif true & then echo e; fi",
            6,
            b"then",
            Term::SemiAmp
        ),
        Some(9)
    );
    assert_eq!(
        find(
            b"if true; then echo a & else echo b; fi",
            3,
            b"else",
            Term::SemiAmp
        ),
        Some(7)
    );
    // The `for` word list: `;` only.
    assert_eq!(find(b"for i in 1 & do :; done", 3, b"do", Term::Semi), None);
    assert_eq!(
        find(b"for i in 1; do :; done", 3, b"do", Term::Semi),
        Some(5)
    );
}

/// The keyword word gate applies to the `&`-preceded row too: a quoted `then`
/// behind a `&` is a command word.
#[test]
fn a_quoted_clause_keyword_behind_the_amp_is_no_keyword() {
    let tokens = tokenize(b"if true & 'then' echo x; fi").unwrap();
    assert_eq!(
        find_preceded_by(
            &tokens,
            1,
            b"then",
            b"if true & 'then' echo x; fi",
            Term::SemiAmp
        ),
        None
    );
}

/// `amp_semi` reads the **trailing** `;` token of each line: true only for a
/// real `;` byte glued behind the plain `&` (task #355). A newline token is
/// never glue (`do echo a &\ndone` is the bash-accepted shape), a word behind
/// the `;` is not the plain `&`, and index 0 has no predecessor.
#[test]
fn amp_semi_is_the_real_semi_byte_behind_the_amp() {
    let glued = |line: &[u8]| {
        let tokens = tokenize(line).unwrap();
        let idx = tokens
            .iter()
            .rposition(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
            .unwrap();
        amp_semi(&tokens, line, idx)
    };
    assert!(glued(b"for i in 1; do echo a & ; done"));
    assert!(!glued(b"for i in 1; do echo a &\ndone"), "a newline token");
    assert!(!glued(b"for i in 1; do echo a ; done"), "behind a word");
    assert!(!glued(b"; fi"), "index 0: no predecessor");
    assert!(!glued(b"a ; fi"), "a word behind the `;`");
    // The `;` byte behind a **glued** `&&` pair is one word, not the plain
    // `&` (the dangling-pair family, task #358): no glue.
    assert!(!glued(b"for i in 1; do echo a && ; done"));
    // Off the end: no token at `idx`.
    let tokens = tokenize(b"for i in 1; do echo a & ; done").unwrap();
    assert!(!amp_semi(
        &tokens,
        b"for i in 1; do echo a & ; done",
        tokens.len()
    ));
}
