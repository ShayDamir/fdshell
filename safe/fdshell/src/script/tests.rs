//! Unit pins of the statement layer's unclosed-block report (task #277).

#![allow(clippy::unwrap_used)]
use super::unclosed::{parse_error, report};
use crate::error::cmd::CmdError;
use crate::error::parse::{ParseError, ParsePosition};

/// The `(line, opener, closer)` rows of the unclosed-block report: the message
/// names the construct the user must close, and `time [-p]` is transparent.
const OPENERS: &[(&[u8], &[u8], &[u8])] = &[
    (b"if true; then echo a", b"if", b"fi"),
    (b"for i in a; do echo a", b"for", b"done"),
    (b"while true", b"while", b"done"),
    (b"until false", b"until", b"done"),
    (b"case 1 in", b"case", b"esac"),
    (b"{ echo a", b"{", b"}"),
    (b"f() {", b"f()", b"}"),
    // The `{` that opens the body ends the opener word, and a group run's own
    // `{` is its first byte, never a terminator.
    (b"f(){ echo a", b"f()", b"}"),
    (b"{echo a", b"{echo", b"}"),
    (b"function f", b"function", b"}"),
    (b"time while true", b"while", b"done"),
    (b"time -p while true", b"while", b"done"),
    // A word the definer gate accepts and no closer keyword follows: the
    // name-validity rule is task #166's, so `$(` stays a definer word here.
    (b"$(", b"$(", b"}"),
];

/// The opener/closer pair the report names, for every block form.
#[test]
fn unclosed_block_names_its_opener_and_closer() {
    for (line, opener, closer) in OPENERS {
        let rep = parse_error(line, 0);
        match rep.current_context() {
            ParseError::MissingBlockCloser {
                opener: got,
                closer: got_closer,
            } => {
                assert_eq!(got.as_bytes().unwrap(), *opener, "{line:?}");
                assert_eq!(got_closer.as_bytes().unwrap(), *closer, "{line:?}");
            }
            other => panic!("{line:?} must report the missing closer, got {other:?}"),
        }
    }
}

/// The report is the statement layer's parse-error class, so the exit status
/// stays the shell-wide 1 of a parse failure.
#[test]
fn unclosed_block_report_is_a_parse_error_context() {
    for (line, _, _) in OPENERS {
        assert!(
            matches!(report(line, 0).current_context(), CmdError::Parse),
            "{line:?}"
        );
    }
}

/// The caret sits on the opener word, so a run with leading whitespace and a
/// transparent `time` prefix points at the keyword that needs closing (the
/// attachment `debug.rs` renders as the caret line).
#[test]
fn caret_points_at_the_opener_word() {
    for (line, want) in [
        (b"  if true; then echo a".as_slice(), 2usize),
        (b"while true".as_slice(), 0),
        (b"  time -p while true".as_slice(), 10),
        (b"\tcase 1 in".as_slice(), 1),
        (b"  { echo a".as_slice(), 2),
    ] {
        let pos = parse_error(line, 0)
            .downcast_ref::<ParsePosition>()
            .unwrap()
            .pos;
        assert_eq!(pos, want, "{line:?}");
    }
}

/// `Never` is never produced for NUL-free input: [`parse_error`]'s invariant
/// arms are unreachable, so the user always gets the named report.
#[test]
fn never_is_never_reported_on_valid_input() {
    for line in [b"if".as_slice(), b"$(", b"", b"f() {", b"time", b"time -p"] {
        assert!(
            !matches!(parse_error(line, 0).current_context(), ParseError::Never),
            "{line:?}"
        );
    }
}
