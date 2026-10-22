#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Unit pins for the pipeline-stage arm of the command-position rule: the word
//! that opens a stage is the one **directly behind the pipe byte**, and the pipe
//! byte is the tokenizer's pipe token ([`crate::parse::pipeline_pipe`]'s rule on
//! raw bytes) — never the token text. A `{`-brace at the position is *not* this
//! rule's (the brace scorer keeps `group_at_run`), and a `case` pattern's
//! alternation `|` never ends a run.

use super::{command_at_run, stage_at_run};

/// The word of `line` that starts at `word` opens a stage?
fn stage(line: &[u8], word: usize) -> bool {
    stage_at_run(line, 0, word)
}

/// A spaced pipe, a glued pipe, and every keyword spelling: the word behind the
/// pipe opens a stage.
#[test]
fn the_word_behind_a_pipe_opens_a_stage() {
    for line in [
        b"echo a | while :; do :; done".as_slice(),
        b"echo a|while :; do :; done".as_slice(),
        b"echo a | if :; then :; fi".as_slice(),
        b"echo a | for x in 1; do :; done".as_slice(),
        b"echo a | until :; do :; done".as_slice(),
        b"echo a | case x in x) :;; esac".as_slice(),
        b"echo a | f() { :; }".as_slice(),
        b"echo a |\twhile :; do :; done".as_slice(),
    ] {
        let word = line.iter().position(|&b| b == b'|').unwrap() + 1;
        let word = line[word..].iter().position(|&b| b != b' ').unwrap() + word;
        assert!(stage(line, word), "{line:?}: the stage word opens");
    }
}

/// Not a stage: a run start, a cond part, and a word that is not the first one
/// behind the pipe (a keyword-shaped *argument* of the stage).
#[test]
fn a_run_start_a_cond_part_and_an_argument_are_not_stages() {
    assert!(!stage(b"while :; do :; done", 0));
    assert!(!stage(b"x && if :; then :; fi", 5));
    assert!(!stage(b"x || if :; then :; fi", 5));
    assert!(!stage(b"echo a | echo while :; do :; done", 13));
    // The empty run: nothing precedes the word, so there is no pipe byte.
    assert!(!stage(b"echo a", 0));
    assert!(!stage(b"echo a", 5));
}

/// The pipe byte's own guards are the tokenizer's: an operator word absorbs it
/// (`>|`, `%>|`, `&>|`), an escape pair shields it (`\|`), a quote span covers
/// it (`"|"`), and a `|` whose twin is a cond operator is the cond-part arm's
/// (LESSONS: the byte-level and token-level rules must agree).
#[test]
fn a_pipe_that_is_not_a_pipeline_pipe_opens_nothing() {
    for (line, word) in [
        (b"echo a>| while :; do :; done".as_slice(), 9usize),
        (b"echo a%>| while :; do :; done", 10),
        (b"echo a&>| while :; do :; done", 10),
        (b"echo a\\| while :; do :; done", 9),
        (b"echo a\"|\" while :; do :; done", 11),
        (b"echo a'|' while :; do :; done", 11),
        (b"echo a|| while :; do :; done", 9),
        (b"echo a || while :; do :; done", 12),
    ] {
        assert!(!stage(line, word), "{line:?}: no pipeline pipe here");
    }
}

/// A quote byte directly *before* the pipe leaves the pipe unquoted: the
/// tokenizer emits its token, and the scanner agrees (`echo x"|"| while …` is
/// two stages, `echo "|" while` is none).
#[test]
fn a_quote_ending_before_the_pipe_is_a_pipe() {
    assert!(stage(b"echo x\"|\"| while :; do :; done", 11));
    assert!(!stage(b"echo \"|\" while :; do :; done", 9));
}

/// The classifier is a *run* rule: it knows nothing about a `case` word list.
/// A glued alternation (`a|while)`) never ends the run, so it is not a stage;
/// a **spaced** one (`a | while)`) is the run's last byte and this layer calls
/// it a stage — the alternation guard is the clause rule's
/// (`keywords::position::after` with `Place::Pattern`), and the measured
/// behaviour stays bash's (`case x in a | while) echo p;; esac` runs the empty
/// `case`, rc 0).
#[test]
fn a_case_pattern_alternation_is_not_this_layers_business() {
    assert!(!stage(b"case x in a|while)".as_slice(), 10));
    assert!(stage(b"case x in a | while)".as_slice(), 14));
}

/// `time [-p]` is not this layer's business: the classifier answers for the word
/// **directly behind the pipe**, so a `time` written there is the stage's word 0
/// (bash rejects `echo a | time while …`, rc 2 — the measured divergence README
/// "Pipelines and lists" pins).
#[test]
fn the_word_directly_behind_the_pipe_is_the_stage_word() {
    assert!(stage(b"echo a | time while :; do :; done", 8));
}

/// The cond layer's single scorer: a run start, a cond part and a stage are all
/// command position; an argument is data.
#[test]
fn command_at_run_covers_all_three_command_arms() {
    assert!(command_at_run(b"if :; then :; fi", 0, 0));
    assert!(command_at_run(b"x && if :; then :; fi", 0, 5));
    assert!(command_at_run(b"echo a | if :; then :; fi", 0, 8));
    assert!(!command_at_run(b"echo if :; then :; fi", 0, 5));
    assert!(!command_at_run(b"echo a>| if :; then :; fi", 0, 9));
}
