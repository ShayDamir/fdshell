#![allow(clippy::unwrap_used)]
use super::*;
use crate::error::parse::ParseError;
use crate::parse::semi::token_range;
use crate::parse::token::tokenize;
use alloc::string::String;

/// The **body text** `trim_body` keeps between the tokens `open` and `closer`
/// of `line`, read back from the line's bytes. The rc/output pins cannot see
/// the kept `;` (a trailing `;`/newline byte is inert for the body's scan), so
/// the rule is pinned on the text itself — that is what kills the
/// byte-check and `&`-arm mutants (task #355).
fn body(line: &[u8], open: &[u8], closer: &[u8]) -> String {
    let tokens = tokenize(line).unwrap();
    let o = tokens
        .iter()
        .position(|(t, _, _, _, _, _)| t.eq_bytes(open))
        .unwrap();
    let c = tokens
        .iter()
        .rposition(|(t, _, _, _, _, _)| t.eq_bytes(closer))
        .unwrap();
    let (s, e) = token_range(trim_body(tokens.get(o + 1..c).unwrap_or(&[]), line));
    String::from_utf8_lossy(line.get(s..e).unwrap_or(b""))
        .into_owned()
        .replace('\n', "\\n")
}

/// The kept `;` is the `;` **byte** glued behind the plain `&`; a newline
/// token is never glue (`do echo a &\ndone` is the bash-accepted
/// `&`-terminated body), a plain `;` after a word is trimmed as ever, and a
/// leading `;` keeps its old trim.
#[test]
fn trim_body_keeps_the_semi_glued_behind_the_amp() {
    assert_eq!(
        body(b"for i in 1; do echo a & ; done", b"do", b"done"),
        "echo a & ;"
    );
    assert_eq!(
        body(b"for i in 1; do echo a & ;\ndone", b"do", b"done"),
        "echo a & ;"
    );
    assert_eq!(
        body(b"for i in 1; do echo a &\ndone", b"do", b"done"),
        "echo a &",
        "the newline token is not glue"
    );
    assert_eq!(
        body(b"for i in 1; do echo a ; done", b"do", b"done"),
        "echo a",
        "the plain trailing `;` still trims"
    );
    assert_eq!(body(b"if true; then ; & fi", b"then", b"fi"), "&");
    assert_eq!(
        body(b"for i in 1; do echo a && ; done", b"do", b"done"),
        "echo a &&",
        "the tokenizer's `&&` is one word, not the plain `&` — the `;` trims"
    );
    assert_eq!(
        body(b"for i in 1; do echo a & ; ; done", b"do", b"done"),
        "echo a & ;",
        "the trim stops at the glue; the `;` before it is out of the slice"
    );
}

/// `body_end` keeps the glued `;` before the closer, on top of the #354 rule
/// (keep the `&`, drop POSIX's `;`, keep a glued word).
#[test]
fn body_end_keeps_the_glued_semi_too() {
    let end = |line: &[u8], closer: &[u8]| {
        let tokens = tokenize(line).unwrap();
        let idx = tokens
            .iter()
            .rposition(|(t, _, _, _, _, _)| t.eq_bytes(closer))
            .unwrap();
        body_end(&tokens, line, idx)
    };
    assert_eq!(end(b"if true; then echo a & ; fi", b"fi"), 8);
    assert_eq!(end(b"if true; then echo a & fi", b"fi"), 7);
    assert_eq!(end(b"if true; then echo a; fi", b"fi"), 6);
}

/// A `&`-terminated condition keeps the #354 cut; the `& ;` form keeps the
/// split text for the condition's own scan (task #355).
#[test]
fn trim_condition_cuts_the_amp_keeps_the_glued_semi() {
    let cond = |line: &[u8], from: usize, to: usize| {
        let tokens = tokenize(line).unwrap();
        let body = trim_condition(tokens.get(from..to).unwrap_or(&[]), line);
        let (s, e) = token_range(body);
        String::from_utf8_lossy(line.get(s..e).unwrap_or(b"")).into_owned()
    };
    assert_eq!(cond(b"if true & then echo x; fi", 1, 3), "true");
    assert_eq!(cond(b"if true & ; then echo x; fi", 1, 4), "true & ;");
    assert_eq!(cond(b"if true; then :; fi", 1, 3), "true");
}

/// The `&`-terminator **flag** of the very slice [`trim_condition`] receives
/// (task #356): the cut `&` is the job's fact and this predicate is its single
/// owner, so the rc/output pins cannot see the predicate's arms — the pair of
/// the newline row and the `;`-byte row is what makes both arms killable
/// (LESSONS #355: a `;` token is a newline in disguise).
#[test]
fn condition_background_reads_the_amp_and_the_semi_byte() {
    let bg = |line: &[u8], from: usize, to: usize| {
        let tokens = tokenize(line).unwrap();
        condition_background(tokens.get(from..to).unwrap_or(&[]), line)
    };
    assert!(bg(b"if true & then echo x; fi", 1, 3));
    assert!(
        bg(b"if true &\nthen echo x; fi", 1, 4),
        "the `;` token is a newline byte, so bash reads it as `& then`"
    );
    assert!(
        bg(b"if true &\n\nthen echo x; fi", 1, 5),
        "the walk steps over every newline token"
    );
    assert!(
        !bg(b"if true & ; then echo x; fi", 1, 4),
        "a real `;` byte stops the walk: that shape stays #355's parse error"
    );
    assert!(
        !bg(b"if true; then :; fi", 1, 3),
        "the plain `;` never sets it"
    );
    assert!(
        !bg(b"if true && & then echo x; fi", 1, 3),
        "the tokenizer's `&&` is one word, not the plain `&`"
    );
    assert!(
        bg(b"if true && & then echo x; fi", 1, 4),
        "the slice ends in the plain `&`; the flag is **inert** here because the \
         cond-list's last part is empty (measured: rc 0, no job, bash rc 2)"
    );
    assert!(
        bg(b"if & then echo x; fi", 1, 2),
        "the single-`&` slice sets the flag; the block layer rejects the shape as \
         an empty condition (task #299), so this row pins the predicate alone"
    );
    assert!(
        !bg(b"if true then echo x; fi", 1, 2),
        "a word ends the walk"
    );
    assert!(!bg(b"if then echo x; fi", 1, 1), "the empty slice is false");
}

/// The condition's non-empty rule (task #299), on the **trimmed** slice: the
/// raw slice between the keywords always carries POSIX's `;`, so only the
/// trimmed slice tells the shapes apart. The `&`-only slice is raw
/// **non**-empty / trimmed empty — that is the row that kills a raw-slice
/// mutant — and the returned text is the trimmed one, pinning the cut.
#[test]
fn block_condition_rejects_a_trimmed_empty_slice() {
    let cond = |line: &[u8], from: usize, to: usize| -> Option<String> {
        let tokens = tokenize(line).unwrap();
        let c = block_condition(
            tokens.get(from..to).unwrap_or(&[]),
            line,
            ParseError::MissingCondition,
        )
        .ok()?;
        let (s, e) = token_range(c);
        Some(String::from_utf8_lossy(line.get(s..e).unwrap_or(b"")).into_owned())
    };
    // Non-empty: a word, a word plus the `&` terminator (cut), a cond-list
    // whose **last part** is empty (not an empty condition).
    assert_eq!(cond(b"if true; then :; fi", 1, 3), Some("true".into()));
    assert_eq!(cond(b"if true & then :; fi", 1, 3), Some("true".into()));
    assert_eq!(
        cond(b"if true && & then :; fi", 1, 4),
        Some("true &&".into())
    );
    // Trimmed-empty: separators only, in every spelling.
    assert_eq!(cond(b"if then :; fi", 1, 1), None, "the empty slice");
    assert_eq!(cond(b"if ; then :; fi", 1, 2), None);
    assert_eq!(cond(b"if ;; then :; fi", 1, 3), None);
    assert_eq!(
        cond(b"if & then :; fi", 1, 2),
        None,
        "raw non-empty (`&`), trimmed empty"
    );
    assert_eq!(cond(b"if ; & then :; fi", 1, 3), None);
}
