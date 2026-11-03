#![allow(clippy::unwrap_used)]
use crate::error::parse::ParseError;
use crate::parse::block_tail::{closer_index, redirects};
use crate::parse::token::tokenize_statement;
use crate::redirect::{RedirectDef, RedirectSource};
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn st(b: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(b.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The walk over `line`'s tail, exactly as a builder's call site runs it: the
/// tokens from the closing keyword `kw` on, and the block `text` the builder was
/// handed (the reconstructed one: the part's bytes, `\n`, the bodies it owns).
fn run(line: &[u8]) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    let text = st(line);
    let tokens = tokenize_statement(text.as_bytes().unwrap()).unwrap();
    let at = closer_index(&tokens, b"done").unwrap();
    redirects(b"done", tokens.get(at..).unwrap_or(&[]), &text)
}

/// `(fd, claimed target)` of each def: the body bytes for a here-doc, the word
/// for a here-string. The claim is what the arm is for, so every row reads it
/// and not just the operator's shape.
fn targets(line: &[u8]) -> Vec<(i32, Vec<u8>)> {
    let defs = run(line).unwrap();
    defs.iter()
        .map(|d| (d.export_to, target(&d.source)))
        .collect()
}

fn target(source: &RedirectSource) -> Vec<u8> {
    match source {
        RedirectSource::HereDoc { body, .. } | RedirectSource::HereString { word: body, .. } => {
            body.as_bytes().unwrap_or(&[]).to_vec()
        }
        other => panic!("not a here-doc/here-string source: {other:?}"),
    }
}

/// Whether the body's `$`/backtick expansion runs — the delimiter's quoting
/// decides it, so the flag is the half of the def the target text cannot show.
fn expands(line: &[u8]) -> Vec<bool> {
    run(line)
        .unwrap()
        .iter()
        .map(|d| match &d.source {
            RedirectSource::HereDoc { expand: true, .. } => true,
            RedirectSource::HereDoc { .. } => false,
            RedirectSource::HereString { .. } => true,
            other => panic!("not a here-doc/here-string source: {other:?}"),
        })
        .collect()
}

fn tail_err(line: &[u8]) -> String {
    match run(line) {
        Ok(defs) => panic!("expected a parse error for {line:?}, got {defs:?}"),
        Err(e) => e.current_context().to_string(),
    }
}

/// One row per here-doc arm spelling, each with its own body region below the
/// command line: the tail slice claims the region of **its own** operator at
/// slice granularity (the `cond_bodies` window/skip pair, LESSONS). The bare
/// `<< word` form consumes the delimiter token, the attached ones do not.
#[test]
fn each_heredoc_spelling_claims_its_own_region() {
    for (line, fd, body) in [
        (
            b"while :; do :; done <<T\nbody\nT".as_slice(),
            0,
            b"body\n".to_vec(),
        ),
        (
            b"while :; do :; done << T\nbody\nT".as_slice(),
            0,
            b"body\n".to_vec(),
        ),
        (
            b"while :; do :; done <<-T\n\tbody\n\tT".as_slice(),
            0,
            b"body\n".to_vec(),
        ),
        (
            b"while :; do :; done 3<<T\nbody\nT".as_slice(),
            3,
            b"body\n".to_vec(),
        ),
    ] {
        assert_eq!(targets(line), vec![(fd, body)], "{line:?}");
    }
}

/// The here-string arm: its word sits on the command line, so no body region is
/// involved — the attached form keeps its word, the bare form takes the next
/// token, and the io-number prefix lands the def on that descriptor.
#[test]
fn the_here_string_arm_takes_its_word_off_the_line() {
    for (line, fd) in [
        (b"while :; do :; done <<<hi".as_slice(), 0),
        (b"while :; do :; done <<< hi".as_slice(), 0),
        (b"while :; do :; done 0<<<hi".as_slice(), 0),
        (b"while :; do :; done 3<<<hi".as_slice(), 3),
    ] {
        assert_eq!(targets(line), vec![(fd, b"hi".to_vec())], "{line:?}");
    }
}

/// The delimiter's quoting decides whether expansion runs in the body: the same
/// rule the simple-command level has, reached through the shared `body_for`.
#[test]
fn the_delimiter_quote_flag_decides_expansion() {
    assert_eq!(
        expands(b"while :; do :; done <<T\n$(echo x)\nT"),
        vec![true]
    );
    assert_eq!(
        expands(b"while :; do :; done <<'T'\n$(echo x)\nT"),
        vec![false]
    );
    assert_eq!(
        targets(b"while :; do :; done <<'T'\n$(echo x)\nT"),
        vec![(0, b"$(echo x)\n".to_vec())]
    );
}

/// A body-position operator written **before** the closer's one on the same
/// physical line owns the first region of the line's stream; the closer's
/// operator claims the second, in global per-line operator order. A claim
/// shifted by one hands the closer `bodyA` and prints the wrong body.
#[test]
fn a_body_position_operator_before_the_closer_keeps_the_global_order() {
    assert_eq!(
        targets(b"while :; do cat <<A; break; done <<T\nbodyA\nA\nbodyT\nT"),
        vec![(0, b"bodyT\n".to_vec())]
    );
}

/// Task #364's rule at this granularity: the delimiter line never arrived, so
/// the operator claims what the stream left — nothing — and the parse accepts it
/// (the end-of-file warning is printed, never returned).
#[test]
fn an_eof_starved_closer_operator_is_accepted_with_an_empty_body() {
    assert_eq!(targets(b"while :; do :; done <<T"), vec![(0, vec![])]);
    assert_eq!(targets(b"while :; do :; done << T"), vec![(0, vec![])]);
}

/// A word behind a heredoc on the closer is task #405's report (bash rc 2, the
/// house convention rc 1); the `word` module's terminator-span exemption is what
/// keeps the **delimiter word** accepted, not this word.
#[test]
fn a_word_behind_a_heredoc_on_the_closer_is_the_report() {
    assert_eq!(
        tail_err(b"while :; do :; done <<T x"),
        "'done' must be followed by a separator (';', a newline, '&', '|') — unexpected word"
    );
}

/// A `<<` glued inside the closer's word opens no word, so the byte scan counts
/// no operator and the tail claims nothing (the pinned house rule, through the
/// keyword-glue exception); the walk's word-break exemption answers the token
/// without a report, so the body lines stay commands.
#[test]
fn a_glued_operator_stays_word_data() {
    assert!(targets(b"while :; do :; done<<T\nbody\nT").is_empty());
    assert!(targets(b"while :; do :; done<<T").is_empty());
}

/// The parse layer owns an operator whose delimiter word never arrived: the
/// house redirect syntax error (bash rc 2), the one `operator_at` gives at the
/// simple-command level.
#[test]
fn an_operator_with_no_delimiter_word_is_the_redirect_error() {
    assert_eq!(
        tail_err(b"while :; do :; done <<"),
        "invalid redirect syntax"
    );
    assert_eq!(
        tail_err(b"while :; do :; done 3<<"),
        "invalid redirect syntax"
    );
}
