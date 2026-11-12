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

/// The gap in front of the body keyword is **at most one `;` and then any run of
/// newlines** (task #400), measured against bash 5.3.9: `)) ;` + newline + `do`,
/// a double newline, `;` + two newlines, and the same run in front of task
/// #394's `{ …; }` body — bash runs every one of them. The byte test is the whole
/// rule: the tokenizer folds a `;` byte and a newline into one `;` token, so only
/// the raw byte tells them apart. The shapes that break the run are pinned below.
#[test]
fn a_run_of_terminators_before_the_body_keyword_is_accepted() {
    for line in [
        b"for ((i=0;i<2;i++)) ;\ndo echo a; done".as_slice(),
        b"for ((i=0;i<2;i++))\n\ndo echo a; done".as_slice(),
        b"for ((i=0;i<2;i++)) ;\n\ndo echo a; done".as_slice(),
        b"for ((i=0;i<2;i++)) ;\n{ echo a; }".as_slice(),
        b"for ((i=0;i<2;i++))\n\n{ echo a; }".as_slice(),
    ] {
        assert_eq!(arith_for(line).body.data, c"echo a".into(), "{line:?}");
    }
}

/// Every gap shape bash rejects (rc 2) stays a parse error here (rc 1, the
/// repo-wide convention), measured: a `;` **behind** a newline — bash gives the
/// tail one terminator slot and absorbs newlines only behind it — a second `;`
/// (`; ;`, the glued `;;`), the `&` family, and the quote/escape spellings that
/// the span gate keeps out of the run (LESSONS: "A separator-shaped word needs
/// the span gate").
#[test]
fn a_gap_that_breaks_the_run_stays_a_parse_error() {
    for line in [
        b"for ((i=0;i<2;i++))\n; do :; done".as_slice(),
        b"for ((i=0;i<2;i++))\n\n; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) ;\n; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) ; ; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) ;; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) & do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) ;& do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) &\ndo :; done".as_slice(),
        b"for ((i=0;i<2;i++)) \";\" do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) \\; do :; done".as_slice(),
        b"for ((i=0;i<2;i++)) ; ; { echo a; }".as_slice(),
        b"for ((i=0;i<2;i++))\n; { echo a; }".as_slice(),
        // The run may exhaust the statement: the walk asks for one token past
        // the last newline and gets nothing, and the keyword gate answers.
        b"for ((i=0;i<2;i++)) ;\n".as_slice(),
    ] {
        assert_eq!(parse_err(line), "expected 'do'", "{line:?}");
    }
}

/// A `#` comment row standing in the gap is not one of the run's terminator tokens,
/// yet bash runs the loop (measured `a a` rc 0 on every row — task #518). The rule
/// lives in the byte layer (`keywords/kind/arith_for`): the tokenizer drops the
/// comment row, so this walk never sees it and needs no arm of its own — which is
/// exactly why the `do`-body rows of the same family already worked. The `;` slot
/// stays **before** the comment arm, and that order is what keeps the
/// comment-**before**-`;` row in the test above rejecting (bash rc 2). The body
/// keyword spelling makes no difference: both bodies share this one rule.
#[test]
fn a_comment_row_in_the_gap_is_bash_parity() {
    for line in [
        b"for ((i=0;i<2;i++)) ;\n# c\ndo echo a; done".as_slice(),
        b"for ((i=0;i<2;i++)) ;\ndo\n# c\necho a\ndone".as_slice(),
        b"for ((i=0;i<2;i++)) ;\n# c\n{ echo a; }".as_slice(),
        b"for ((i=0;i<2;i++))\n# c\n{ echo a; }".as_slice(),
        b"for ((i=0;i<2;i++)) ;\n# c\n# d\n{ echo a; }".as_slice(),
    ] {
        assert_eq!(arith_for(line).body.data, c"echo a".into(), "{line:?}");
    }
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

/// A plain `&` token anywhere in the `in` list is the async operator, not a
/// word: the builder rejects the list with the `do` expectation (task #360,
/// bash stops at rc 2 with its own `do` message). The four spellings below pin
/// the gate at the unit layer; the integration family in `tests/loops.rs`
/// carries the rest of the shapes.
///
/// * `& ;` — the `;` is the **list** separator, so the `&` used to ride into
///   the list as a word (the pin #360 flipped from #355).
/// * `& ; do; done` — the **ordering** pin: bash reports the earliest offending
///   token, so the gate precedes the closer/body validators.
/// * `;&;` — the tokenizer emits `;` and `&` as separate tokens, so the `&`
///   survives `trim_semi` and the gate must read the **untrimmed** slice.
/// * a `&` **mid**-list — the predicate is `any`, not `all`.
#[test]
fn a_plain_ampersand_in_the_word_list_is_a_parse_error() {
    for line in [
        b"for i in 1 & ; do echo x; done".as_slice(),
        b"for i in 1 & ; do; done".as_slice(),
        b"for i in 1 ;&; do echo x; done".as_slice(),
        b"for i in 1 & 2 ; do echo x; done".as_slice(),
    ] {
        assert_eq!(parse_err(line), "expected 'do'", "{line:?}: bash rc 2");
    }
}

/// The span gate is the whole quoting rule: `"&"`, `'&'` and a glued `x&` are
/// word **data**, and an escaped `\&` reaches the list as the pair — the
/// tokenizer keeps the backslash in the word text (measured, and bash prints
/// `&` for the quoted and escaped forms). The paren twins are task **#470**'s
/// parity rows: a quoted `"(a)"` keeps its mask bit and an escaped `\(` keeps
/// the backslash as the leading byte, so neither fires the word-opening reading
/// (bash rc 0 for both, measured — a bare closing `)` on the escaped row would
/// be its own separator token and join the rejects instead).
/// LESSONS: "A separator-shaped word needs the span gate"
#[test]
fn a_quoted_ampersand_in_the_word_list_stays_a_word() {
    for (line, want) in [
        (
            b"for i in \"&\" ; do echo x; done".as_slice(),
            vec![c"&".into()],
        ),
        (
            b"for i in '&' ; do echo x; done".as_slice(),
            vec![c"&".into()],
        ),
        (
            b"for i in \\& ; do echo x; done".as_slice(),
            vec![c"\\&".into()],
        ),
        (
            b"for i in x& ; do echo x; done".as_slice(),
            vec![c"x&".into()],
        ),
        (
            b"for i in 1 \"&\" 2 ; do echo x; done".as_slice(),
            vec![c"1".into(), c"&".into(), c"2".into()],
        ),
        (
            b"for i in \"(a)\" ; do echo x; done".as_slice(),
            vec![c"(a)".into()],
        ),
        (
            b"for i in \\(a ; do echo x; done".as_slice(),
            vec![c"\\(a".into()],
        ),
    ] {
        let ParsedLine::For(fb) = crate::parse::parse(&stext(line)).unwrap() else {
            panic!("expected For {line:?}")
        };
        assert_eq!(fb.words, want, "{line:?}");
    }
}

/// Task #524 extended that gate to the whole operator-word set, read through the
/// ONE predicate `parse::word_quoted::operator_word`: `&&`, the pipeline `|` (and
/// the `||` that the tokenizer spells as two `|` tokens) and a `;` that is not the
/// list's own terminator before the `do` all end the list, and the builder answers
/// with the `do` expectation where bash stops at rc 2.
///
/// Task **#470** decided the parens: the word-opening `(` (the tokenizer's word
/// byte) and the bare `)` (the tokenizer's own separator token) join the rejects,
/// and the position owns nothing now — the `(a` row below is the move that took
/// this test's words' assertion. The owners' half of the rule stays load-bearing
/// through the `case` word slot's `;` (`word_quoted/tests.rs` pins the parameter).
#[test]
fn an_operator_word_in_the_word_list_is_a_parse_error() {
    for line in [
        b"for i in 1 && ; do echo x; done".as_slice(),
        b"for i in 1 | ; do echo x; done".as_slice(),
        b"for i in 1 || 2; do echo x; done".as_slice(),
        b"for i in 1 ; 2; do echo x; done".as_slice(),
        b"for i in ; 1; do echo x; done".as_slice(),
        b"for i in (a); do echo x; done".as_slice(),
    ] {
        assert_eq!(parse_err(line), "expected 'do'", "{line:?}: bash rc 2");
    }
}

/// bash's `for_action` ends with a compound list, so a group opener written behind
/// the region is the loop's **body** (task #394). The group parse answers it — its
/// terminator rule, its emptiness rule and its trailing redirections are the group's
/// own, not a copy of the `do … done` tail.
#[test]
fn a_group_behind_the_region_is_the_body() {
    for line in [
        b"for ((i=0;i<2;i++)) { echo a; }".as_slice(),
        b"for ((i=0;i<2;i++)); { echo a; }".as_slice(),
    ] {
        let b = arith_for(line);
        assert_eq!(b.body.data, c"echo a".into(), "{line:?}");
    }
    // The body's trailing redirection is the block's, exactly as `done > f`'s is
    // (task #298).
    assert_eq!(
        arith_for(b"for ((i=0;i<2;i++)) { echo a; } > f")
            .redirects
            .len(),
        1
    );
    // The group's own rules answer the group's own shapes.
    assert_eq!(
        parse_err(b"for ((i=0;i<2;i++)) { echo a }"),
        "expected ';' before '}'"
    );
    assert_eq!(parse_err(b"for ((i=0;i<2;i++)) { ; }"), "group is empty");
    // A pipe behind the body is the group tail's pinned divergence (#186/#190); the
    // two-separator pair and a quoted opener keep the parse layer's `do` rule.
    assert_eq!(
        parse_err(b"for ((i=0;i<2;i++)) { echo a; } | cat"),
        "unexpected pipe"
    );
    assert_eq!(
        parse_err(b"for ((i=0;i<2;i++)) ;; { echo a; }"),
        "expected 'do'"
    );
    assert_eq!(
        parse_err(b"for ((i=0;i<2;i++)) ; ; { echo a; }"),
        "expected 'do'"
    );
    assert_eq!(
        parse_err(b"for ((i=0;i<2;i++)) \"{ echo a; }\""),
        "expected 'do'"
    );
}

// --- the word form's clause with no `in` (task #416) -----------------------

/// Parse a whole statement, expecting the **word form** `for`.
fn word_for(line: &[u8]) -> ForBlock {
    match crate::parse::parse(&stext(line)).unwrap() {
        ParsedLine::For(fb) => fb,
        _ => panic!("not a word-form for: {line:?}"),
    }
}

/// POSIX #2.9.1 makes the clause's `in compound-list` **optional**, and bash
/// then reads `"$@"`: the clause is `for name` alone, the parse carries that
/// fact as a field, and `words` is empty — the run layer takes the list from the
/// positional deque (`for_run/tests.rs` is the run-side pin). The newline
/// spelling of the terminator is the same clause, and so is the glued `do`
/// spelling — POSIX's `newline_list` may be **empty** behind the name (task
/// #521, `for i do …` runs at bash rc 0) — and the keyword-name family (`in`,
/// `fi`, `do`, `then`) are **names** here: the scan parks every word behind
/// `for` and never scores it — the family's `done` member, the block's own
/// closer, included (task #522, pinned below).
#[test]
fn a_clause_without_in_has_no_words_and_the_default_flag() {
    for (line, name) in [
        (b"for i; do echo x; done".as_slice(), b"i".as_slice()),
        (b"for i\ndo echo x; done".as_slice(), b"i".as_slice()),
        (b"for in; do echo x; done".as_slice(), b"in".as_slice()),
        (b"for fi; do echo x; done".as_slice(), b"fi".as_slice()),
        (b"for do; do echo x; done".as_slice(), b"do".as_slice()),
        (b"for then; do echo x; done".as_slice(), b"then".as_slice()),
        (b"for i do echo x; done".as_slice(), b"i".as_slice()),
        (b"for in do echo x; done".as_slice(), b"in".as_slice()),
        (b"for fi do echo x; done".as_slice(), b"fi".as_slice()),
        (b"for do do echo x; done".as_slice(), b"do".as_slice()),
        (b"for then do echo x; done".as_slice(), b"then".as_slice()),
    ] {
        let fb = word_for(line);
        assert!(fb.words_default_to_positional, "{line:?}");
        assert!(fb.words.is_empty(), "{line:?}");
        assert!(fb.var.eq_bytes(name), "{line:?}");
        assert_eq!(fb.body.data, c"echo x".into(), "{line:?}");
    }
}

/// A **word** sitting where the optional `in` would be stays the head's error:
/// the clause is `for name` only when the slot behind the name holds the clause
/// terminator or the empty-`newline_list` clause keyword `do` (task #521),
/// through the span gates (task #247), so a quoted `"in"`/`";"`/`"do"` word is
/// word data and a real word (`for x a`, `for i 1 2`) is bash's rc-2 family —
/// rc 1 here, the repo-wide convention. The glued `for i do …` spelling is
/// parity now and is pinned above and in `tests/loops.rs`.
#[test]
fn a_word_where_the_optional_in_belongs_stays_the_heads_error() {
    for (line, bash_rc) in [
        (b"for i \"in\"; do echo x; done".as_slice(), 2),
        (b"for i \"in\" a; do echo x; done".as_slice(), 2),
        (b"for in done; do echo x; done".as_slice(), 2),
        (b"for i &; do echo x; done".as_slice(), 2),
        (b"for i \";\"; do echo x; done".as_slice(), 2),
        (b"for i \"do\" do echo x; done".as_slice(), 2),
        (b"for i 1 2 do echo x; done".as_slice(), 2),
    ] {
        assert_eq!(
            parse_err(line),
            "expected 'in'",
            "{line:?}: bash rc {bash_rc}"
        );
    }
}

/// The pair that makes the flag load-bearing: `for name in; do` is the **empty**
/// list — zero iterations (bash rc 0; task #299 pins it green against bash) — and
/// parses with no words and the flag `false`.
#[test]
fn the_in_form_still_distinguishes_an_empty_list() {
    let fb = word_for(b"for i in; do echo a; done");
    assert!(!fb.words_default_to_positional);
    assert!(fb.words.is_empty());
    assert_eq!(fb.var, c"i".into());
    assert_eq!(fb.body.data, c"echo a".into());
}

/// The block's **own** closer written in the name slot is the **name**, like the
/// rest of the keyword-name family (`in`, `fi`, `do`, `then`) — POSIX #2.9.1
/// puts a *name* behind `for`, and the closing walk is seeded in the word list
/// (task #522, `semi/closing/walk.rs`), so no word behind the opener — the name
/// slot included — is ever scored as the closer. All three spellings parse:
/// the name is `done`, the body is `echo x`, and the word-list facts keep their
/// shapes — the `in` row has the word `a` and the flag `false`, the wordless
/// and the glued rows (tasks #416/#521) have an empty list and the flag `true`.
/// Bash 5.3.9 runs every row at rc 0; the end-to-end pins are in
/// `tests/loops.rs`.
#[test]
fn a_closer_shaped_name_of_a_for_clause_is_the_name() {
    let fb = word_for(b"for done in a; do echo x; done");
    assert!(fb.var.eq_bytes(b"done"));
    assert_eq!(fb.words, vec![c"a".into()]);
    assert!(!fb.words_default_to_positional);
    assert_eq!(fb.body.data, c"echo x".into());
    let fb = word_for(b"for done; do echo x; done");
    assert!(fb.var.eq_bytes(b"done"));
    assert!(fb.words.is_empty());
    assert!(fb.words_default_to_positional);
    assert_eq!(fb.body.data, c"echo x".into());
    let fb = word_for(b"for done do echo x; done");
    assert!(fb.var.eq_bytes(b"done"));
    assert!(fb.words.is_empty());
    assert!(fb.words_default_to_positional);
    assert_eq!(fb.body.data, c"echo x".into());
}

/// The word at `anchor` — the **name** word of a wordless clause — is the only
/// data that can separate `anchor + 1` (the operator gate's word-list slice)
/// from its `−`/`×` mutants. The gate strips the slice's **last** token (the
/// list's own terminator, `for_block/words.rs`), so an operator-shaped name is
/// word data the gate never sees when the slot behind it is **empty** — the
/// glued spelling runs at rc 0 (task #521's arm is what makes it reachable);
/// bash rc 2 there, where its tokenizer eats the `&`. The killer is the `;`
/// spelling: the mutant's slice is then [`&`, `;`], the strip leaves [`&`], and
/// the gate answers `expected 'do'`. The `in`-form row pins the other side: its
/// anchor word is the `in` keyword, never an operator word, so the gate's
/// answer cannot move — bash rc 2 on both, the tokenizer again.
#[test]
fn an_operator_shaped_name_of_a_wordless_clause_stays_the_name() {
    let fb = word_for(b"for & do echo x; done");
    assert!(fb.var.eq_bytes(b"&"));
    assert!(fb.words_default_to_positional);
    assert!(fb.words.is_empty());
    assert_eq!(fb.body.data, c"echo x".into());
    let fb = word_for(b"for & ; do echo x; done");
    assert!(fb.var.eq_bytes(b"&"));
    assert!(fb.words_default_to_positional);
    assert!(fb.words.is_empty());
    assert_eq!(fb.body.data, c"echo x".into());
    let fb = word_for(b"for & in 1; do echo x; done");
    assert!(fb.var.eq_bytes(b"&"));
    assert_eq!(fb.words, vec![c"1".into()]);
    assert!(!fb.words_default_to_positional);
    assert_eq!(fb.body.data, c"echo x".into());
}

/// `for i do; do echo x; done` — bash rejects the row at rc 2 (the `;` breaks
/// its clause, which needs a `newline_list` behind the first `do`). Task #521's
/// empty-`newline_list` arm takes the **first** bare `do` as the body keyword,
/// so the second one is body data and the row now **parses**: name `i`, the
/// positional-default flag, an empty word list, body `do echo x` — which the
/// run layer executes as the command `do` (`do` not found, rc 1, measured with
/// `set -- a`; nothing runs with no positionals). Pinned as measured so the
/// divergence cannot drift silently.
#[test]
fn a_second_do_behind_the_empty_newline_list_is_body_data() {
    let fb = word_for(b"for i do; do echo x; done");
    assert!(fb.words_default_to_positional);
    assert!(fb.words.is_empty());
    assert!(fb.var.eq_bytes(b"i"));
    assert_eq!(fb.body.data, c"do echo x".into());
}
