#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Parse pins of the arithmetic `for ((init;cond;step)) ; do … ; done`
//! (task #293), measured against bash 5.3.9.

use super::*;
use crate::parse::ParsedLine;
use crate::parse::token::tokenize_statement;
use alloc::string::{String, ToString};
use alloc::vec;
use sys::{Origin, Position, ScriptText};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The clause's raw text, the only part of it a pin below asserts on (its
/// position and origin are the statement's, not the clause's own).
fn text(clause: &Option<ScriptText>) -> Option<sys::ShortCStr> {
    clause.as_ref().map(|t| t.data.clone())
}

/// Parse a whole statement, expecting the arithmetic `for`.
fn arith_for(line: &[u8]) -> arith::ArithForBlock {
    match crate::parse::parse(&stext(line)).unwrap() {
        ParsedLine::ArithFor(block) => block,
        _ => panic!("not an arithmetic for: {line:?}"),
    }
}

/// `detect` on a fragment: the shape a stage slice, a mid-run part, or a
/// statement the script splitter never completes hands to this parser.
fn detect(line: &[u8]) -> Result<Option<arith::ArithForBlock>, Report<ParseError>> {
    let raw = tokenize_statement(line).unwrap();
    arith::detect(&raw, &stext(line))
}

/// The statement's parse-error message.
fn parse_err(line: &[u8]) -> String {
    crate::parse::parse(&stext(line))
        .err()
        .unwrap()
        .current_context()
        .to_string()
}

/// The three clauses are the region's raw texts, split at the top-level `;`,
/// and the body is the `do … done` text.
#[test]
fn the_three_clauses_are_the_raw_texts() {
    let b = arith_for(b"for ((i=0;i<3;i++)); do echo $i; done");
    assert_eq!(text(&b.init), Some(c"i=0".into()));
    assert_eq!(text(&b.cond), Some(c"i<3".into()));
    assert_eq!(text(&b.step), Some(c"i++".into()));
    assert_eq!(b.body.data, c"echo $i".into());
}

/// Whitespace inside the region survives (the clause texts are raw), and the
/// gap between `for` and `((` may be any run of whitespace.
#[test]
fn spaces_and_tabs_survive_inside_the_region() {
    let b = arith_for(b"for (( i = 0 ; i < 2 ; i++ ))\t;\tdo echo $i;\tdone");
    assert_eq!(text(&b.init), Some(c"i = 0".into()));
    assert_eq!(text(&b.cond), Some(c"i < 2".into()));
    assert_eq!(text(&b.step), Some(c"i++".into()));
    let tabbed = arith_for(b"for\t((i=0;i<2;i++)) do echo $i; done");
    assert_eq!(text(&tabbed.init), Some(c"i=0".into()));
}

/// Bash accepts `))do` (no separator at all between the region and `do`).
#[test]
fn do_may_be_glued_after_the_close_paren() {
    let b = arith_for(b"for ((i=0;i<2;i++))do echo $i;done");
    assert_eq!(text(&b.step), Some(c"i++".into()));
    assert_eq!(b.body.data, c"echo $i".into());
}

/// A newline before `do` is a list terminator, as the word form's.
#[test]
fn do_may_follow_a_newline() {
    let b = arith_for(b"for ((i=0;i<2;i++))\ndo echo $i\ndone");
    assert_eq!(b.body.data, c"echo $i".into());
}

/// The `;` between the region and `do` is optional: `)) do` and `)) ; do`.
#[test]
fn a_semicolon_before_do_is_optional() {
    assert_eq!(
        arith_for(b"for ((i=0;i<2;i++)) do echo $i; done").body.data,
        c"echo $i".into()
    );
    assert_eq!(
        arith_for(b"for ((i=0;i<2;i++)) ; do echo $i; done")
            .body
            .data,
        c"echo $i".into()
    );
}

/// A clause written empty is `None`: bash runs `for ((;;))` as the
/// empty-init/true-cond/no-step loop, spaced spelling included.
#[test]
fn empty_clauses_are_none() {
    for line in [
        b"for ((;;)) do echo x; break; done".to_vec(),
        b"for (( ; ; )) ; do echo x; break; done".to_vec(),
    ] {
        let b = arith_for(&line);
        assert_eq!(text(&b.init), None, "{line:?}");
        assert_eq!(text(&b.cond), None, "{line:?}");
        assert_eq!(text(&b.step), None, "{line:?}");
        assert_eq!(b.body.data, c"echo x; break".into(), "{line:?}");
    }
}

/// The comma operator is clause data: one clause, not several.
#[test]
fn comma_clause_stays_one_clause() {
    let b = arith_for(b"for ((i=1,j=2;i<3;i++)); do echo \"$i $j\"; done");
    assert_eq!(text(&b.init), Some(c"i=1,j=2".into()));
    assert_eq!(text(&b.cond), Some(c"i<3".into()));
}

/// The `;` inside a `$( … )` clause sits at paren depth, so it is not a cut;
/// a nested `(` / `)` pair likewise hides its bytes.
#[test]
fn semicolon_inside_command_substitution_is_not_a_clause_cut() {
    let b = arith_for(b"for ((i=0;i<$(echo 2; echo 3);i++)) do echo $i; done");
    assert_eq!(text(&b.cond), Some(c"i<$(echo 2; echo 3)".into()));
    assert_eq!(text(&b.step), Some(c"i++".into()));
}

#[test]
fn paren_depth_inside_a_clause_is_respected() {
    let b = arith_for(b"for ((i=(0);i<2;i++)) do echo $i; done");
    assert_eq!(text(&b.init), Some(c"i=(0)".into()));
}

/// `<<` inside the region is bash's shift, not a heredoc operator: the region is
/// opaque, and the body's own `<<` claims its body bytes exactly as the word
/// form's does (task #273's append mode).
#[test]
fn shift_inside_the_region_is_not_a_heredoc() {
    let b = arith_for(b"for ((i=0;i<(1<<2);i++)); do cat <<B; done\nbodyB\nB");
    assert_eq!(text(&b.cond), Some(c"i<(1<<2)".into()));
    assert_eq!(b.body.data, c"cat <<B\nbodyB\nB".into());
}

/// A newline is not a clause cut: the `;`-terminated spelling parses with three
/// clauses (their texts keep the newlines as whitespace), and the
/// newline-separated one is a clause-count error (bash: `syntax error:
/// arithmetic expression required`).
#[test]
fn newline_is_not_a_clause_cut() {
    let b = arith_for(b"for ((i=0;\ni<2;\ni++)) do echo $i; done");
    assert_eq!(text(&b.init), Some(c"i=0".into()));
    assert_eq!(text(&b.cond), Some(c"i<2".into()));
    assert_eq!(text(&b.step), Some(c"i++".into()));
    assert_eq!(
        parse_err(b"for ((i=0\ni<2\ni++)) do echo x; done"),
        "`for ((expr;expr;expr))` needs exactly two `;` separators"
    );
}

/// The clause count is exactly three, and bash rejects every other count with
/// the same shape (`syntax error: arithmetic expression required`).
#[test]
fn the_form_needs_exactly_two_separators() {
    for line in [
        b"for ((i=0))".as_slice(),
        b"for ((i=0;i<2))".as_slice(),
        b"for ((i=0;))".as_slice(),
        b"for ((i=0;i<2;i++;))".as_slice(),
        b"for (( in 1 2 ))".as_slice(),
    ] {
        assert_eq!(
            parse_err(line),
            "`for ((expr;expr;expr))` needs exactly two `;` separators",
            "{line:?}"
        );
    }
}

/// A region the line never closes is a parse error — of the arithmetic form,
/// never an infinite loop.
#[test]
fn unclosed_region_is_a_parse_error() {
    assert_eq!(
        parse_err(b"for ((i=0;i<2"),
        "`for ((expr;expr;expr))` needs a closing `))`"
    );
    // The same fragment at the token layer (a stage slice, a mid-run part).
    assert!(detect(b"for ((i=0;i<2;i+i").is_err());
}

/// Any word other than `do` right after the region is bash's rc-2 family: a
/// glued word, a redirection, the `;;` pair, a second `;`, a pipe.
#[test]
fn word_after_the_region_is_not_do() {
    for line in [
        b"for ((i=0;i<2;i++))x; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) 2>/dev/null; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) ;; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) ; ; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) | cat".as_slice(),
    ] {
        assert_eq!(parse_err(line), "expected 'do'", "{line:?}");
    }
}

/// The tail's own rules are the word form's, reused verbatim: no `done`, no
/// terminator before it, and an empty `do` body.
#[test]
fn the_tail_rules_are_the_word_forms() {
    assert_eq!(
        parse_err(b"for ((i=0;i<2;i++)); do echo a;"),
        "expected 'done'"
    );
    assert_eq!(
        parse_err(b"for ((i=0;i<2;i++)); do done"),
        "expected ';' before 'done'"
    );
    assert_eq!(
        parse_err(b"for ((i=0;i<2;i++)); do; done"),
        "'do' body is empty"
    );
    // A block closer that is not `done` (`}`, `fi`) is a **word** to the
    // closing-keyword walk: the walk is seeded with the `for`'s own `done` (the
    // pairing of task #309), a foreign closer mismatches, no closing keyword is
    // found at all, and the builder's own tail rule answers. Same message as the
    // type-blind walk gave (which stopped at the `-1` delta); the mechanism is
    // now the pairing, so this row is the walk's **no-closer** arm.
    for line in [
        b"for ((i=0;i<2;i++)); do echo a; }".as_slice(),
        b"for ((i=0;i<2;i++)); do echo a; fi".as_slice(),
        b"echo a | for ((i=0;i<2;i++)); do echo a; }".as_slice(),
    ] {
        assert_eq!(parse_err(line), "expected 'done'", "{line:?}");
    }
}

/// `detect` steps aside for every shape that is not the arithmetic form, so the
/// word form (and the plain command) keeps the statement.
#[test]
fn not_the_arithmetic_form() {
    for line in [
        b"for x in a; do :; done".as_slice(),
        b"for (i=0;;) do :; done".as_slice(),
        b"for (();do;done".as_slice(),
        b"for i in 1 2; do :; done".as_slice(),
    ] {
        assert!(detect(line).unwrap().is_none(), "{line:?}");
    }
    // A quoted `for` is a command word, so dispatch never reaches this parser.
    for line in [
        b"'for' ((i=0;i<2;i++)); do :; done".as_slice(),
        b"fo\"r\" ((i=0;i<2;i++)); do :; done".as_slice(),
        b"\\for ((i=0;i<2;i++)); do :; done".as_slice(),
    ] {
        assert!(
            matches!(
                crate::parse::parse(&stext(line)).unwrap(),
                ParsedLine::Cmd(_)
            ),
            "{line:?} must stay a command"
        );
    }
}

/// The word form is untouched by the new gate: the same `ParsedLine::For` with
/// the same var, words and body.
#[test]
fn word_form_pins_are_untouched() {
    let ParsedLine::For(fb) =
        crate::parse::parse(&stext(b"for var in a b c; do echo $var; done")).unwrap()
    else {
        panic!("expected For")
    };
    assert_eq!(fb.var, c"var".into());
    assert_eq!(fb.words, vec![c"a".into(), c"b".into(), c"c".into()]);
    assert_eq!(fb.body.data, c"echo $var".into());
}
