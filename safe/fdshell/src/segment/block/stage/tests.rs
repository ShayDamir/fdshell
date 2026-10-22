#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use alloc::format;
use alloc::string::String;

/// The run text the stage arm sees: a command, the pipeline byte, then `block`
/// as the stage's own text.
fn line(block: &str) -> String {
    format!("echo a | {block}")
}

/// The word right behind `"echo a | "` — the stage arm's own position. Returns
/// the resume offset in the whole line, asserting it is a strict advance that
/// never passes the end of input.
fn stage(block: &str) -> Option<usize> {
    let text = line(block);
    let resume = skip_at(text.as_bytes(), b"echo a | ".len())?;
    assert!(
        resume > b"echo a | ".len() && resume <= text.len(),
        "resume {resume} for {text}"
    );
    Some(resume)
}

/// Score the word starting at `at` of `line`: `Some(resume)` when it opens a
/// stage block, `None` when it opens none.
fn skip_at(line: &[u8], at: usize) -> Option<usize> {
    let mut book = Book::new();
    book.start = 0;
    book.i = at;
    skip(line, &mut book)
}

/// A keyword block written as a pipeline stage is skipped **whole**: the span
/// ends at the separator that closes the closing keyword's run, so the `;`
/// after the closer still ends the statement, and what is written between the
/// closer and that separator (`done && echo y`) stays in the run for the
/// pipeline and cond layers to re-split. An unclosed span resumes at the end.
#[test]
fn a_stage_block_skips_its_whole_span() {
    for block in ["while :; do :; done", "if :; then echo t; fi"] {
        assert_eq!(stage(block), Some(line(block).len()), "{block}");
    }
    assert_eq!(
        stage("while :; do :; done; echo tail"),
        Some(b"echo a | while :; do :; done".len())
    );
    assert_eq!(stage("if :; then echo t; fi; echo y"), Some(30));
    // An operator behind the closer with no separator after it: the run keeps
    // it all, so the span runs to the end of input.
    assert_eq!(
        stage("if :; then echo t; fi && echo y"),
        Some(b"echo a | if :; then echo t; fi && echo y".len())
    );
    assert_eq!(stage("while :; do :;"), Some(23));
}

/// Quotes and comments inside the stage's block belong to its span: a `; done`
/// inside a double-quoted word and a comment naming a closer never end it
/// early, and a quoted opener word opens no block at all.
#[test]
fn quotes_and_comments_stay_inside_the_span() {
    for block in [
        r#"while :; do echo "; done"; break; done"#,
        "while :; do # done\n :; break; done",
    ] {
        assert_eq!(stage(block), Some(line(block).len()), "{block}");
    }
    assert_eq!(stage(r#"echo "while" :; do :; done"#), None);
}

/// The arm needs the **pipeline** byte: a run-start keyword, a cond part
/// behind `&&`, a keyword-shaped argument, and the `||` pair (the cond arm's
/// own, `segment/block/midrun.rs`) are all `None` here.
#[test]
fn only_a_word_behind_a_pipeline_byte_is_a_stage() {
    assert_eq!(skip_at(b"while :; do :; done", 0), None);
    assert_eq!(skip_at(b"x && while :; do :; done", 5), None);
    assert_eq!(skip_at(b"echo while :; do :; done", 5), None);
    assert_eq!(skip_at(b"echo a || if :; then :; fi", 9), None);
}

/// The byte test is the tokenizer's pipe rule: a quoted `"|"`, an escaped
/// `\|`, and a `|` absorbed into a `>`-terminated operator word stay word data
/// (LESSONS: the byte- and token-level rules must agree).
#[test]
fn a_quoted_escaped_or_absorbed_pipe_is_word_data() {
    // The glued `>`-terminated words start at 5 (the whole `a>|while` is one
    // word); the quoted/escaped forms put the keyword word at 11.
    for (text, at) in [
        (r#"echo a "|" while :; do :; done"#, 11),
        (r"echo a \| while :; do :; done", 11),
        ("echo a>|while :; do :; done", 5),
        ("echo a&>|while :; do :; done", 5),
        ("echo a%>|while :; do :; done", 5),
    ] {
        assert_eq!(skip_at(text.as_bytes(), at), None, "{text}");
    }
}

/// A `name() { … }` definition opens a stage too (its span runs to the closing
/// brace), and the event-case `wait` block keeps the shared scorer's gate: the
/// POSIX builtin (`wait` with no pattern keyword behind it) opens no span, so
/// the stage's own `;` splits as before.
#[test]
fn a_function_definition_is_a_span_and_the_builtin_gate_holds() {
    for block in [
        "f() { echo b; }",
        "f() { echo b;",
        "wait\n readable %rd) echo r ;;\ndone",
    ] {
        assert_eq!(stage(block), Some(line(block).len()), "{block}");
    }
    assert_eq!(stage("wait; echo tail"), None);
}

/// `time` behind the `|` seeds the span exactly as it does behind `&&` (the
/// shared seed), while the *parse* layer keeps it as the stage's word 0 and
/// runs it as a command: `echo a | time while …` is rc 127, where bash reports
/// a syntax error rc 2 and `x && time while …` runs the loop.
#[test]
fn time_seeds_the_span_and_stays_the_stages_word_zero() {
    for block in ["time while :; do :; done", "time -p while :; do :; done"] {
        assert_eq!(stage(block), Some(line(block).len()), "{block}");
    }
}
