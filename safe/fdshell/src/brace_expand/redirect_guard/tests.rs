//! The `worddef` guard on the operand region, arm by arm (task #324). Every
//! expected message is the operand **as written** and every row was measured
//! against bash 5.3.9 `bash -c` on 2026-10-26: a two-word operand bails, a
//! one-word operand passes, a zero-word operand bails, and the
//! here-doc/pidvar/argument arms never reach the rule.

#![allow(clippy::unwrap_used)]
use super::guard;
use crate::error::cmd::CmdError;
use crate::parse::token::tokenize_statement;
use alloc::format;
use alloc::string::String;
use error_stack::Report;

/// Run the guard on token `i` of `line`, with the token's raw span.
fn guard_line(line: &str, i: usize) -> Result<(), Report<CmdError>> {
    let tokens = tokenize_statement(line.as_bytes()).unwrap();
    let (_t, start, end, _fq, _mask, _spans) = tokens.get(i).unwrap();
    let span = line.as_bytes().get(*start..*end).unwrap();
    guard(&tokens, i, span)
}

fn bails(line: &str, i: usize, name: &str) {
    let err = guard_line(line, i).unwrap_err();
    match err.current_context() {
        CmdError::AmbiguousRedirect { name: n } => {
            let got = String::from_utf8(n.as_bytes().unwrap().to_vec()).unwrap();
            assert_eq!(got, name, "{line:?} token {i}");
        }
        other => panic!("{line:?} token {i}: expected AmbiguousRedirect, got {other:?}"),
    }
}

fn passes(line: &str, i: usize) {
    guard_line(line, i).unwrap_or_else(|e| panic!("{line:?} token {i}: {e:?}"));
}

/// The attached path arms: every operator spelling, and a brace glued before or
/// after the group, each naming the whole operand.
#[test]
fn a_two_word_path_operand_bails_with_the_operand_as_written() {
    for (line, name) in [
        ("cat >{a,b}", "{a,b}"),
        ("cat >>{a,b}", "{a,b}"),
        ("cat <>f{a,b}", "f{a,b}"),
        ("cat >|{a,b}", "{a,b}"),
        ("cat 2>{a,b}", "{a,b}"),
        ("cat >x{a,b}", "x{a,b}"),
        ("cat >{a,b}x", "{a,b}x"),
        ("cat >{a,b}c{d,e}", "{a,b}c{d,e}"),
        ("cat >{1..3}", "{1..3}"),
        ("cat >{a,b*}", "{a,b*}"),
        ("cat >{a,b}\"c\"", "{a,b}\"c\""),
        ("cat >'a'b{c,d}", "'a'b{c,d}"),
        ("cat >{a,$n}", "{a,$n}"),
    ] {
        bails(line, 1, name);
    }
}

/// The `<&word` / `>&word` dup arms and the `%var` arms. `{,3}` has one
/// non-empty word, so bash proceeds to the fd and reports `Bad file
/// descriptor`; fdshell reports its own fd message (rc parity, task #370 owns
/// the operand expansion itself).
#[test]
fn a_two_word_dup_or_var_operand_bails() {
    for (line, name) in [
        ("cat <&{3,4}", "{3,4}"),
        ("cat >&{3,4}", "{3,4}"),
        ("cat 2>&{3,4}", "{3,4}"),
        ("cat <&{3..5}", "{3..5}"),
        ("cat <&{3,-}", "{3,-}"),
        ("cat <&{3,{4,5}}", "{3,{4,5}}"),
        ("cat <&{3,4}x", "{3,4}x"),
        ("cat >&%{a,b}", "%{a,b}"),
        ("cat >%{a,b}", "%{a,b}"),
        ("cat 3>%{a,b}", "%{a,b}"),
    ] {
        bails(line, 1, name);
    }
    passes("cat <&{,3}", 1);
    passes("cat >&- ", 1);
}

/// The `&>` family and the bare forms, whose operand is the next token.
#[test]
fn the_combined_and_bare_forms_are_guarded_too() {
    bails("echo x &>{a,b}", 2, "{a,b}");
    bails("echo x &>>{a,b}", 2, "{a,b}");
    bails("echo x &>{a,b}x", 2, "{a,b}x");
    bails("cat > {a,b}", 2, "{a,b}");
    bails("cat >> {a,b}", 2, "{a,b}");
    bails("cat <> {a,b}", 2, "{a,b}");
    bails("cat >| {a,b}", 2, "{a,b}");
    bails("cat 2> {a,b}", 2, "{a,b}");
    bails("cat < {a,b}", 2, "{a,b}");
    bails("echo x &> {a,b}", 3, "{a,b}");
    bails("cat > >{a,b}", 2, "{a,b}");
}

/// Bash counts **non-empty** words: `{,zz}` is one word (`zz`), `{,}` and
/// `{,,}` are zero words, and both zero-word shapes are ambiguous.
#[test]
fn the_count_is_of_non_empty_words() {
    passes("cat >{,zz}", 1);
    passes("cat > {,zz}", 2);
    bails("cat >{,}", 1, "{,}");
    bails("cat >{,,}", 1, "{,,}");
}

/// The arms that must stay untouched: a single expanded word (the rows bash
/// opens), quoted braces, the protected here-doc and here-string words, the
/// pidvar arm, and words that are not redirects at all.
#[test]
fn one_word_and_the_operandless_words_pass() {
    passes("cat >{a}", 1);
    passes("cat >{a..a}", 1);
    passes("cat >x{a}y", 1);
    passes("cat >'{a,b}'", 1);
    passes("cat >{a\\,b}", 1);
    passes("cat >{ 1 , 2 }", 1);
    passes("cat <<{a,b}", 1);
    passes("cat <<< {a,b}", 2);
    passes("echo x &>&p", 2);
    passes("echo x>{a,b}", 1);
    passes("exec {a,b}>f", 1);
    passes("cat 2'>'{a,b}", 1);
    passes("echo {a,b}", 1);
}

/// The guard's own error type carries the name; the chain is one frame, so a
/// consumer sees exactly `CmdError::AmbiguousRedirect` (STYLE §4.16).
#[test]
fn the_bail_is_one_frame() {
    let err = guard_line("cat >{a,b}", 1).unwrap_err();
    let frames = format!("{err:?}");
    assert!(frames.contains("ambiguous redirect: {a,b}"), "{frames}");
}
