#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

#[test]
fn bare_run_before_the_brace_opens_a_group() {
    assert_eq!(group_at_run(b"{ echo a; }", 0, 0), Some(true));
    assert_eq!(group_at_run(b"   { echo a; }", 0, 3), Some(true));
}

#[test]
fn time_prefix_run_opens_a_group() {
    assert_eq!(group_at_run(b"time { echo a; }", 0, 5), Some(true));
    assert_eq!(group_at_run(b"time -p { echo a; }", 0, 8), Some(true));
    assert_eq!(group_at_run(b"time  -p  { echo a; }", 0, 10), Some(true));
    assert_eq!(group_at_run(b"time time -p { echo a; }", 0, 13), Some(true));
    assert_eq!(group_at_run(b"  time { echo a; }", 2, 7), Some(true));
}

#[test]
fn command_words_before_the_brace_are_word_data() {
    assert_eq!(group_at_run(b"echo { a; }", 0, 5), None);
    assert_eq!(group_at_run(b"time echo { a; }", 0, 10), None);
    assert_eq!(group_at_run(b"timeless { a; }", 0, 8), None);
    assert_eq!(group_at_run(b"echo x{ a; }", 0, 6), None);
}

#[test]
fn cond_operator_before_the_brace_starts_a_cond_part() {
    assert_eq!(group_at_run(b"echo a && { echo b; }", 0, 10), Some(false));
    assert_eq!(group_at_run(b"echo a || { echo b; }", 0, 10), Some(false));
}

#[test]
fn time_prefix_trailing_a_cond_operator_starts_a_cond_part() {
    assert_eq!(
        group_at_run(b"echo a && time { echo b; }", 0, 15),
        Some(false)
    );
    assert_eq!(
        group_at_run(b"echo a || time -p { echo b; }", 0, 18),
        Some(false)
    );
    // A whole prefix *run* trails the operator: the cond-part test strips as
    // much as the statement-side test does (`time time -p`, not one word).
    assert_eq!(
        group_at_run(b"echo a && time time -p { echo b; }", 0, 23),
        Some(false)
    );
    assert_eq!(
        group_at_run(b"echo a && time time { echo b; }", 0, 20),
        Some(false)
    );
    assert_eq!(
        group_at_run(b"echo a || time -p time { echo b; }", 0, 23),
        Some(false)
    );
    // The prefix must sit *after* the operator: a word before it is data.
    assert_eq!(
        group_at_run(b"echo time -p && time { b; }", 0, 21),
        Some(false)
    );
    assert_eq!(
        group_at_run(b"time time -p x && { b; }", 0, 18),
        Some(false)
    );
    assert_eq!(group_at_run(b"timeless -p && { b; }", 0, 15), Some(false));
}

#[test]
fn span_start_is_the_brace_without_a_prefix() {
    assert_eq!(span_start(b"echo a && { b; }", 0, 10), 10);
    assert_eq!(span_start(b"{ b; }", 0, 0), 0);
}
#[test]
fn span_start_includes_a_bare_time_prefix() {
    assert_eq!(span_start(b"time { echo a; }", 0, 5), 0);
    assert_eq!(span_start(b"time -p { echo a; }", 0, 8), 0);
    // A whole prefix run written alone in the run is inside the span too.
    assert_eq!(span_start(b"time time -p { echo a; }", 0, 13), 0);
    // The span starts at the prefix word, not at its leading whitespace.
    assert_eq!(span_start(b" && time { b; }", 3, 9), 4);
    assert_eq!(span_start(b" && time time -p { b; }", 3, 17), 4);
    // A prefix with arguments after it is not the group's prefix.
    assert_eq!(span_start(b"time echo { b; }", 0, 10), 10);
    assert_eq!(span_start(b"time time -p echo { b; }", 0, 18), 18);
}

// Task #235: the definer run rule (cond layer only — `group_at_run` stays
// `None` there so the segment scanner keeps claiming the definition).
#[test]
fn definer_runs_opens_the_body_group() {
    for (line, start, brace) in [
        (b"f() { echo a; }".as_slice(), 0, 4),
        (b"f(){ echo a; }".as_slice(), 0, 3),
        (b"function f { echo a; }".as_slice(), 0, 11),
        (b"function f() { echo a; }".as_slice(), 0, 13),
        (b"function f () { echo a; }".as_slice(), 0, 14),
        (b"function f ( ) { echo a; }".as_slice(), 0, 15),
        (b"f() ( ) { echo a; }".as_slice(), 0, 8),
        (b"f() (){ echo a; }".as_slice(), 0, 6),
        // An indented run, and a run that starts after `;`.
        (b"  f() { echo a; }".as_slice(), 2, 6),
        (b"x=1; f() { echo a; }".as_slice(), 5, 9),
    ] {
        assert!(definer_at_run(line, start, brace), "{line:?}");
    }
}

#[test]
fn definer_mid_run_after_a_cond_operator_opens_the_body_group() {
    for (line, brace) in [
        (b"true && f() { echo a; }".as_slice(), 12),
        (b"true && function f { echo a; }".as_slice(), 19),
        (b"false || f() { echo a; }".as_slice(), 13),
        // The scan is quote-blind: the last raw `&&` ends the tail.
        (b"echo 'a && b' && f() { echo a; }".as_slice(), 21),
    ] {
        assert!(definer_at_run(line, 0, brace), "{line:?}");
    }
}

/// The gate's `start` alignment (task #247): `definer_at_run` trims the run, so
/// it measures the definer words from the byte the trimmed text starts at. A
/// `start` left at the run's untrimmed position shifts the quote gate by the
/// leading whitespace — the verdict would flip with the indentation width, and a
/// `start + end` → `start * end` shift of the mid-run walk is only measurable at
/// `start > 0`.
#[test]
fn definer_gate_measures_the_run_from_its_own_first_byte() {
    for (line, start, brace) in [
        // An indented definer, a wide gap, and a mid-run definer whose run
        // starts after `;` and behind `&&`.
        (b"   function f { echo hi; }".as_slice(), 3, 14),
        (b"   function  f { echo hi; }".as_slice(), 3, 15),
        (b"echo a; true && function f { echo a; }".as_slice(), 8, 27),
    ] {
        assert!(definer_at_run(line, start, brace), "{line:?}");
    }
    for (line, start, brace) in [
        // The same runs with a quoted definer word: no definer at any start.
        (b"   function 'f' { echo hi; }".as_slice(), 3, 16),
        (b"   f'oo'() { echo hi; }".as_slice(), 3, 11),
        (
            b"echo a; true && function  'f' { echo a; }".as_slice(),
            8,
            30,
        ),
    ] {
        assert!(!definer_at_run(line, start, brace), "{line:?}");
    }
}

#[test]
fn non_definer_runs_are_no_body_openers() {
    for (line, start, brace) in [
        // The plain group keeps `group_at_run`'s answer: an empty run.
        (b"{ echo a; }".as_slice(), 0, 0),
        (b"echo { a; }".as_slice(), 0, 5),
        (b"echo x { a; }".as_slice(), 0, 7),
        // A cond part before the brace is `group_at_run`'s business, not a definer.
        (b"echo a && { b; }".as_slice(), 0, 10),
        // POSIX split parens: the definer rule rejects the shape (LESSONS
        // "Byte-level definer words must be the tokenizer's tokens").
        (b"f ( ) { echo a; }".as_slice(), 0, 6),
        // A *word* filler never joins a definer run: bash rejects these with
        // rc 2 and fdshell keeps the parse error visible.
        (b"f() echo { a && b; }".as_slice(), 0, 9),
        (b"f() x( ) { echo a; }".as_slice(), 0, 9),
        (b"function ) f { echo a; }".as_slice(), 0, 13),
        (b"function)f { echo a; }".as_slice(), 0, 11),
        (b"f} { echo a; }".as_slice(), 0, 3),
        // `time` is not a definer, and an operator ends the run, not a definer.
        (b"time f() { echo a; }".as_slice(), 0, 9),
        (b"f() && { b; }".as_slice(), 0, 7),
    ] {
        assert!(!definer_at_run(line, start, brace), "{line:?}");
    }
}
