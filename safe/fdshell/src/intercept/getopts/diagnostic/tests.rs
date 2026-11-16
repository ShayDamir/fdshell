#![allow(clippy::unwrap_used)]
use super::*;
use crate::state::ShellState;
use alloc::vec::Vec;

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

/// `$0` as the builder takes it — **as given** (plan §3.3). Stubbed so the pinned
/// bytes do not depend on the launch seed (in-process it is `fdshell`, a `sh -c`
/// launch prints `sh`).
fn prog() -> ShortCStr {
    s(b"fdshell")
}

fn bytes(message: &ShortCStr) -> Vec<u8> {
    message.as_bytes().unwrap().to_vec()
}

#[test]
fn the_verbose_unknown_option_row_prints_the_offender_verbatim() {
    // Measured `set -- -x; getopts "ab:c" o` on bash 5.3.9.
    let message = message(false, &Problem::Unknown, &prog(), b'x').unwrap();
    assert_eq!(bytes(&message), b"fdshell: illegal option -- x\n".to_vec());
}

#[test]
fn the_verbose_missing_argument_row_names_the_option_character() {
    // Measured `set -- -b; getopts "ab:c" o`: the **optstring label** `b`, not the
    // offending character — and the shell **does** print this row (the plan §2 table
    // as re-measured on task591).
    let message = message(false, &Problem::MissingArgument, &prog(), b'b').unwrap();
    assert_eq!(
        bytes(&message),
        b"fdshell: option requires an argument -- b\n".to_vec()
    );
}

#[test]
fn the_silent_mode_prints_nothing() {
    // Both silent rows answer `None`; the character they name and the `OPTARG` they
    // write are the driver's rows, pinned in `intercept/getopts/tests.rs`.
    assert!(message(true, &Problem::Unknown, &prog(), b'x').is_none());
    assert!(message(true, &Problem::MissingArgument, &prog(), b'b').is_none());
}

#[test]
fn a_non_ascii_offender_reaches_the_message_as_that_byte() {
    // Plan §3.3: every message is assembled from **bytes**, so the byte arrives
    // verbatim — a lossy path would write the replacement character instead.
    // Measured `set -- -\xc3; getopts "ab:c" o` on bash 5.3.9.
    let message = message(false, &Problem::Unknown, &prog(), 0xC3).unwrap();
    assert_eq!(
        bytes(&message),
        b"fdshell: illegal option -- \xC3\n".to_vec()
    );
}

#[test]
fn the_program_name_is_printed_as_given() {
    // A launch as `sh -c` prints `sh:` (the pre-existing `$0` rule, not this task's);
    // the builder splices whatever it is handed, and drops bash's `line N:` field.
    let message = message(false, &Problem::Unknown, &s(b"sh"), b'x').unwrap();
    assert_eq!(bytes(&message), b"sh: illegal option -- x\n".to_vec());
}

#[test]
fn the_identifier_message_keeps_bashs_shape_without_the_line_field() {
    // bash's own line carries `line 1:` before `getopts:`; fdshell's builtin
    // diagnostics carry no script-line prefix (plan §3.3), so the message is the rest.
    assert_eq!(
        bytes(&invalid_identifier(&prog(), &s(b"1abc"))),
        b"fdshell: getopts: `1abc': not a valid identifier\n".to_vec()
    );
}

#[test]
fn the_usage_line_has_no_shell_prefix() {
    // Measured `getopts "ab:c"` and `getopts` on bash 5.3.9: the line is printed
    // **without** any shell prefix, and it is the only message here with none.
    assert_eq!(
        bytes(&usage()),
        b"getopts: usage: getopts optstring name [arg ...]\n".to_vec()
    );
}

#[test]
fn a_shell_state_prints_under_its_own_name() {
    // The in-process seed of `state/ctor.rs` — the shape the run-level
    // `/dev/full`-and-scratch pins in `intercept/getopts/tests.rs` observe.
    let state = ShellState::new();
    let message = message(
        false,
        &Problem::Unknown,
        &state.program_name.value.clone(),
        b'x',
    )
    .unwrap();
    assert_eq!(bytes(&message), b"fdshell: illegal option -- x\n".to_vec());
}
