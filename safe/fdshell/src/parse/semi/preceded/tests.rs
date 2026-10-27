#![allow(clippy::unwrap_used)]
use super::*;
use crate::parse::token::tokenize;
use alloc::string::ToString;

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

/// The `;`-before-closer predicate (task #301): a real `;` byte and the
/// newline token are the terminator, the plain `&` is one too (task #354), and
/// the closer of a compound command is a legal predecessor — bash makes the `;`
/// after a compound command optional. A glued cond pair (`&&`), a pipeline `|`,
/// an ordinary word, a **glued** closer (`}>f`, `esac>f`: bash rejects
/// `while false; do { :; }>f done` rc 2, measured) and a quoted `"fi"` word are
/// not.
#[test]
fn a_closer_predecessor_is_a_terminator_or_a_closer() {
    // `line` ends at the block's closer; the probe is the token in front of it.
    let before_closer = |line: &[u8]| {
        let tokens = tokenize(line).unwrap();
        is_closer_predecessor(&tokens, line, tokens.len() - 2)
    };
    assert!(before_closer(b"while :; do :; done"), "the `;` byte");
    assert!(
        before_closer(b"while :; do :\ndone"),
        "the newline tokenizes as a `;`"
    );
    assert!(
        before_closer(b"for i in 1; do echo a & done"),
        "the plain `&`"
    );
    for line in [
        b"while false; do if :; then :; fi done".as_slice(),
        b"if true; then { echo a; } fi".as_slice(),
        b"while false; do case x in a) echo c ;; esac done".as_slice(),
        b"if true; then ( echo a ) fi".as_slice(),
        b"if true; then while :; do break; done fi".as_slice(),
    ] {
        assert!(
            before_closer(line),
            "a compound command's closer precedes: {line:?}"
        );
    }
    for line in [
        b"if true; then echo a && fi".as_slice(),
        b"if true; then echo a || fi".as_slice(),
        b"while false; do : | done".as_slice(),
        b"while false; do echo a done".as_slice(),
        b"if true; then b fi".as_slice(),
        b"while false; do { :; }>f done".as_slice(),
        b"while false; do case x in a) ;; esac>f done".as_slice(),
        b"if true; then \"fi\" fi".as_slice(),
        b"if true; then 'done' fi".as_slice(),
    ] {
        assert!(
            !before_closer(line),
            "no terminator and no closer in front of: {line:?}"
        );
    }
    // Index 0 is the block's own opener, never a predecessor; off the list is
    // `false`.
    let line = b"if true; then fi";
    let tokens = tokenize(line).unwrap();
    assert!(!is_closer_predecessor(&tokens, line, 0));
    assert!(!is_closer_predecessor(&tokens, line, tokens.len()));
}

/// [`ensure_before_closer`] reports `err` exactly when the predicate is false,
/// and passes a form whose closer stands behind a compound command's closer
/// (the rule `for` pinned first, task #301 extends it to every block form).
#[test]
fn ensure_before_closer_reports_the_predicate_failure() {
    let check = |line: &'static [u8]| {
        let tokens = tokenize(line).unwrap();
        ensure_before_closer(
            &tokens,
            line,
            tokens.len() - 1,
            ParseError::ExpectedSemicolonBeforeFi,
        )
    };
    assert!(check(b"if true; then :; fi").is_ok());
    assert!(check(b"if true; then { echo a; } fi").is_ok());
    for line in [
        b"if true; then fi".as_slice(),
        // The glued cond pair is one `&&` token, not the plain `&` (task #358's
        // flipped rows land on this rule; bash rc 2, measured).
        b"if true; then echo a && fi".as_slice(),
        b"if true; then echo a else fi".as_slice(),
        b"if true; then echo a >f fi".as_slice(),
    ] {
        let err = check(line).err();
        assert_eq!(
            err.map(|r| r.current_context().to_string()).as_deref(),
            Some("expected ';' before 'fi'"),
            "{line:?} must report the missing `;`"
        );
    }
}

/// Every separator word of the terminator set is read through [`span_word`]
/// (task #336): a quoted `";"` / `"&"` word is **word data**, so it terminates
/// no compound list, precedes no clause keyword, is no `& ;` glue and is no
/// legal closer predecessor. Each row is paired with its unquoted twin, so a
/// predicate that drops the span gate is dead.
#[test]
fn a_quoted_terminator_word_terminates_no_list() {
    let find = |line: &[u8], term: Term| {
        find_preceded_by(&tokenize(line).unwrap(), 1, b"then", line, term)
    };
    // The real terminator finds the keyword; the quoted words do not, in either
    // term set.
    assert_eq!(find(b"if true; then :; fi", Term::Semi), Some(3));
    assert_eq!(find(b"if true \";\" then :; fi", Term::Semi), None);
    assert_eq!(find(b"if true ';' then :; fi", Term::Semi), None);
    assert_eq!(find(b"if true \";\" then :; fi", Term::SemiAmp), None);
    assert_eq!(find(b"if true \"&\" then :; fi", Term::SemiAmp), None);
    assert_eq!(find(b"if true \"&&\" then :; fi", Term::SemiAmp), None);
    assert_eq!(find(b"if true & then :; fi", Term::SemiAmp), Some(3));
    assert_eq!(
        find(b"if true \"&\" & then :; fi", Term::SemiAmp),
        Some(4),
        "the **unquoted** `&` behind the quoted word terminates the list"
    );

    // [`is_list_terminator`] on the word itself: the newline and `)` tokens (the
    // tokenizer emits a `;` token for each) are terminators, the quoted words
    // are words.
    let last = |line: &[u8]| {
        let tokens = tokenize(line).unwrap();
        let n = tokens.len();
        is_list_terminator(tokens.get(n - 1).unwrap())
    };
    assert!(last(b"echo a ;"));
    assert!(last(b"echo a\n"), "the newline tokenizes as a `;`");
    assert!(
        !last(b"echo a )"),
        "the `)` word is a closer, not a terminator"
    );
    assert!(last(b"echo a &"));
    assert!(!last(b"echo a \";\""));
    assert!(!last(b"echo a ';'"));
    assert!(!last(b"echo a \"&\""));
    assert!(!last(b"echo a \\;"), "the escape pair stays in the word");

    // [`amp_semi`]: a quoted `";"` glued behind the real `&` is word data, and
    // a real `;` behind a quoted `"&"` is no glue — the `&` half is the
    // discriminating row (the `;` half's byte is a quote, so the byte arm
    // already refused it).
    let glued = |line: &[u8]| {
        let tokens = tokenize(line).unwrap();
        let idx = tokens
            .iter()
            .rposition(|(t, _, _, _, _, _)| t.eq_bytes(b";"))
            .unwrap();
        amp_semi(&tokens, line, idx)
    };
    assert!(glued(b"for i in 1; do echo a & ; done"));
    assert!(!glued(b"for i in 1; do echo a \"&\" ; done"));
    assert!(!glued(b"for i in 1; do echo a & \";\" done"));

    // [`is_closer_predecessor`]: the quoted word leaves the closer with no legal
    // predecessor, so every block form reports its own missing `;`.
    let before_closer = |line: &[u8]| {
        let tokens = tokenize(line).unwrap();
        is_closer_predecessor(&tokens, line, tokens.len() - 2)
    };
    assert!(before_closer(b"if true; then echo a ; fi"));
    assert!(before_closer(b"if true; then echo a & fi"));
    assert!(!before_closer(b"if true; then echo a \";\" fi"));
    assert!(!before_closer(b"if true; then echo a ';' fi"));
    assert!(!before_closer(b"if true; then echo a \"&\" fi"));
}
