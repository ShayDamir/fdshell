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
