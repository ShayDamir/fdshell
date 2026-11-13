#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::token::tokenize_statement;
use crate::parse::umask_mode::UmaskMode;

fn detect_of(line: &[u8]) -> (Option<UmaskMode>, bool, bool) {
    let tokens = tokenize_statement(line).unwrap();
    match detect(&tokens).unwrap() {
        Some(ParsedLine::Umask {
            mode,
            dash_p,
            dash_s,
        }) => (mode, dash_p, dash_s),
        _ => panic!("expected Umask for {line:?}"),
    }
}

fn fails_with(line: &[u8], pred: impl Fn(&ParseError) -> bool) -> bool {
    let tokens = tokenize_statement(line).unwrap();
    match detect(&tokens) {
        Err(err) => pred(err.current_context()),
        Ok(_) => panic!("{line:?} parsed"),
    }
}

/// The `-p` option (task #472), now beside its `-S` twin: both are accepted before the
/// operand, may repeat and cluster (XBD Utility Syntax Guidelines), and they only
/// choose the **form** of the no-operand report — which the run prints.
#[test]
fn the_options_come_before_the_operand() {
    for (line, mode, dash_p, dash_s) in [
        (b"umask" as &[u8], None, false, false),
        (b"umask -p", None, true, false),
        (b"umask -p -p", None, true, false),
        (b"umask -S", None, false, true),
        (b"umask -S -S", None, false, true),
        (b"umask -p -S", None, true, true),
        (b"umask -S -p", None, true, true),
        (b"umask -pS", None, true, true),
        (b"umask -Sp", None, true, true),
        (b"umask -p 022", Some(UmaskMode::Octal(0o022)), true, false),
        (b"umask -S 022", Some(UmaskMode::Octal(0o022)), false, true),
        (b"umask 022", Some(UmaskMode::Octal(0o022)), false, false),
        (b"umask 0o077", Some(UmaskMode::Octal(0o077)), false, false),
    ] {
        assert_eq!(detect_of(line), (mode, dash_p, dash_s), "line {line:?}");
    }
}

/// A symbolic operand is a mode, not an option, and it reaches the run as the AST the
/// run evaluates (`symbolic/tests.rs` follows the AST to bash's masks).
#[test]
fn a_symbolic_operand_is_the_mode_operand() {
    for line in [
        &b"umask u=rwx"[..],
        b"umask a=rwx",
        b"umask g-w",
        b"umask u=rwx,g=,o=",
        b"umask -S u=rwx",
        b"umask -p u=rwx",
    ] {
        assert!(
            matches!(detect_of(line), (Some(UmaskMode::Symbolic(_)), _, _)),
            "line {line:?}"
        );
    }
    // A bare `-` is an operand, not an option: bash's no-op clause whose operator is
    // the `-` itself. `--` ends the options and takes no operand of its own.
    assert!(matches!(
        detect_of(b"umask -"),
        (Some(UmaskMode::Symbolic(_)), false, false)
    ));
    assert_eq!(detect_of(b"umask --"), (None, false, false));
    assert!(matches!(
        detect_of(b"umask -- -x"),
        (Some(UmaskMode::Symbolic(_)), false, false)
    ));
}

/// Decision D3 of task #472, kept: a second operand is reported where bash ignores it
/// silently — including a flag **after** the operand, which is that second operand.
#[test]
fn a_second_operand_is_reported() {
    for line in [
        &b"umask 022 extra"[..],
        b"umask 022 -p",
        b"umask -p 022 extra",
        b"umask u=rwx extra",
        b"umask u=rwx -S",
    ] {
        assert!(
            fails_with(line, |e| matches!(
                e,
                ParseError::UmaskTakesAtMostOneArgument
            )),
            "line: {line:?}"
        );
    }
}

/// A flag letter that is not `p` or `S` is the option error, naming the word (bash
/// names the letter and answers its usage error at rc 2 — fdshell's parse errors are
/// rc 1). A lone `--` is not a flag, and a `--` that ends the options leaves the next
/// word as the operand, so `-x` there is the symbolic clause `-x`.
#[test]
fn an_unknown_flag_is_reported_on_its_word() {
    for (line, flag) in [
        (b"umask -x" as &[u8], &b"-x"[..]),
        (b"umask -Sq", &b"-Sq"[..]),
        (b"umask -P", &b"-P"[..]),
    ] {
        assert!(
            fails_with(line, |e| matches!(e,
                ParseError::InvalidUmaskOption { flag: f } if f.eq_bytes(flag))),
            "line {line:?}"
        );
    }
}
