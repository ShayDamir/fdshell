use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;
use sys::signalfd;

use crate::error::cmd::CmdError;
use crate::intercept::signalfd_cmd::signals;
use crate::state::{EXIT, ShellState, TrapDisposition};

use super::{cond, list, parse};

/// A condition operand, as the shell would hand it over after expansion.
fn word(text: &str) -> ShortCStr {
    ShortCStr::from_vec(text.as_bytes().to_vec()).unwrap()
}

/// A list-form line, as text — the tests compare whole lines, because the line
/// **is** the contract: what it prints has to parse again.
fn line(cond: i32, disposition: &TrapDisposition) -> String {
    let mut out = Vec::new();
    list::push_line(cond, disposition, &mut out).unwrap();
    String::from_utf8(out).unwrap()
}

fn condition(text: &str) -> i32 {
    cond::condition(&word(text)).unwrap()
}

fn bad(text: &str) -> bool {
    let report: Report<CmdError> = cond::condition(&word(text)).unwrap_err();
    matches!(report.current_context(), CmdError::TrapBadCondition { .. })
}

fn reserved(text: &str) -> bool {
    let report: Report<CmdError> = cond::condition(&word(text)).unwrap_err();
    matches!(
        report.current_context(),
        CmdError::TrapReservedSignal { .. }
    )
}

fn usage(fields: &[&str]) -> bool {
    let fields: Vec<ShortCStr> = fields.iter().map(|f| word(f)).collect();
    let report: Report<CmdError> = parse::form(&fields).unwrap_err();
    matches!(report.current_context(), CmdError::TrapUsage)
}

fn form(fields: &[&str]) -> parse::Request {
    let fields: Vec<ShortCStr> = fields.iter().map(|f| word(f)).collect();
    parse::form(&fields).unwrap().unwrap()
}

// ---- the condition grammar -------------------------------------------------

/// `EXIT` and `0` are the same condition (POSIX XCU 2.15), and the shared store's
/// slot for it is key `0` — the only key that is not a signal.
#[test]
fn exit_and_zero_are_the_same_slot() {
    assert_eq!(condition("EXIT"), EXIT);
    assert_eq!(condition("0"), EXIT);
}

/// A name and its number agree, and the `SIG` prefix the list form prints is
/// accepted — the printed line is a registration, not an error (bash's extension,
/// measured; case is NOT folded, so `usr1` is not a condition).
#[test]
fn names_numbers_and_the_printed_prefix_agree() {
    assert_eq!(condition("USR1"), signalfd::SIGUSR1);
    assert_eq!(condition("SIGUSR1"), signalfd::SIGUSR1);
    assert_eq!(condition("5"), signalfd::SIGTRAP);
    assert_eq!(condition("TRAP"), signalfd::SIGTRAP);
}

/// The whole table round-trips in both directions: every name the parser accepts
/// resolves, the printer spells it back the way the parser reads it, and the
/// printed `SIG` form is accepted too. `STOP` is the one row the reserved-pair test
/// owns, so it is not listed here.
#[test]
fn the_table_round_trips_both_ways() {
    for name in [
        "HUP", "INT", "QUIT", "ILL", "TRAP", "ABRT", "BUS", "FPE", "SEGV", "PIPE", "ALRM", "TERM",
        "CHLD", "CONT", "TSTP", "TTIN", "TTOU", "URG", "XCPU", "XFSZ", "VTALRM", "PROF", "WINCH",
        "IO", "PWR", "SYS", "USR1", "USR2",
    ] {
        let sig = condition(name);
        assert_ne!(sig, EXIT, "{name} is not the EXIT slot");
        assert_eq!(
            condition(&format!("SIG{name}")),
            sig,
            "the printed form re-inputs"
        );
        assert_eq!(
            signals::signal_name(sig).map(|b| b.to_vec()),
            Some(name.as_bytes().to_vec()),
            "the printer must spell {name} as the parser reads it"
        );
    }
}

/// An operand that is neither a name nor a usable number is reported, not fatal —
/// including a lowercase name and the numbers outside the range.
#[test]
fn an_operand_outside_the_table_is_a_clean_error() {
    assert!(bad("BADSIG"));
    assert!(bad("usr1"));
    assert!(bad("999"));
    assert!(bad("-3"));
    assert!(bad("SIGEXIT"));
}

/// SIGKILL and SIGSTOP: POSIX leaves trapping them undefined, fdshell rejects them
/// — by name in both spellings and by number (bash accepts them silently, so this
/// is a deliberate divergence).
#[test]
fn the_reserved_pair_is_rejected() {
    assert!(reserved("KILL"));
    assert!(reserved("SIGKILL"));
    assert!(reserved("STOP"));
    assert!(reserved("SIGSTOP"));
    assert!(reserved("9"));
    assert!(reserved("19"));
}

// ---- the form grammar -----------------------------------------------------

/// No action operand at all is the list form — and the `--` marker the list form
/// prints is consumed, so `trap --` on its own is the list form too.
#[test]
fn no_action_is_the_list_form() {
    let fields: Vec<ShortCStr> = vec![];
    assert!(parse::form(&fields).unwrap().is_none());
    assert!(parse::form(&[word("--")]).unwrap().is_none());
}

/// Registration: the action and the conditions it applies to, one action shared by
/// every condition (bash prints one line per condition, measured).
#[test]
fn registration_keeps_the_action_and_every_condition() {
    let request = form(&["echo x", "USR1", "INT"]);
    assert_eq!(request.disposition, TrapDisposition::Action(word("echo x")));
    assert_eq!(request.conditions, vec![word("USR1"), word("INT")]);
}

/// The empty action is POSIX's ignore; `-` is the reset; and a first operand that
/// is a decimal integer makes every operand a condition and the action a reset.
#[test]
fn the_ignore_and_reset_forms() {
    let request = form(&["", "USR1"]);
    assert_eq!(request.disposition, TrapDisposition::Ignore);
    assert_eq!(request.conditions, vec![word("USR1")]);

    let request = form(&["-", "USR1"]);
    assert_eq!(request.disposition, TrapDisposition::Default);
    assert_eq!(request.conditions, vec![word("USR1")]);

    let request = form(&["5", "INT"]);
    assert_eq!(request.disposition, TrapDisposition::Default);
    assert_eq!(request.conditions, vec![word("5"), word("INT")]);

    let request = form(&["0"]);
    assert_eq!(request.disposition, TrapDisposition::Default);
    assert_eq!(request.conditions, vec![word("0")]);
}

/// An action with nothing to apply it to — including the `-` and null-action forms
/// — is a usage error, and bash's measured answer is 2 (pinned by the integration
/// test that also shows the script carries on).
#[test]
fn an_action_without_conditions_is_a_usage_error() {
    assert!(usage(&["-"]));
    assert!(usage(&[""]));
    assert!(usage(&["echo x"]));
    assert!(usage(&["--", "echo x"]));
}

// ---- the list form --------------------------------------------------------

/// A registered action prints a re-inputtable line, with the shared table's name
/// and the shared quoting rule — the same line the parser takes back in.
#[test]
fn a_registered_action_prints_a_re_inputtable_line() {
    let state = ShellState::new();
    assert_eq!(
        line(signalfd::SIGUSR1, &TrapDisposition::Action(word("echo x"))),
        "trap -- 'echo x' SIGUSR1\n"
    );
    assert!(state.trap_list().is_empty(), "the fresh table is empty");
}

/// POSIX's null action prints an empty pair of quotes (the store keeps it as
/// `Ignore`, so this is the ignore form's own line, not an absent entry).
#[test]
fn the_ignore_form_prints_an_empty_action() {
    assert_eq!(
        line(signalfd::SIGINT, &TrapDisposition::Ignore),
        "trap -- '' SIGINT\n"
    );
}

/// An embedded `'` is the reason the quoting rule is one owner's job: the printed
/// line must re-input as the same action (bash's `'\''`, measured).
#[test]
fn an_embedded_quote_is_rendered_for_re_input() {
    assert_eq!(
        line(
            signalfd::SIGUSR1,
            &TrapDisposition::Action(word("echo a'b"))
        ),
        "trap -- 'echo a'\\''b' SIGUSR1\n"
    );
}

/// The table's slot prints as `EXIT`, and a condition the shared table has no name
/// for prints as its number — both re-input (bash prints `32` for the one it
/// cannot name, measured).
#[test]
fn the_exit_slot_and_a_nameless_number_both_print() {
    assert_eq!(
        line(EXIT, &TrapDisposition::Action(word("x"))),
        "trap -- 'x' EXIT\n"
    );
    assert_eq!(
        line(34, &TrapDisposition::Action(word("x"))),
        "trap -- 'x' 34\n"
    );
}

/// An absent entry is the default, and the default prints no line (measured: bash
/// prints nothing after resetting everything).
#[test]
fn an_absent_entry_prints_nothing() {
    assert_eq!(line(signalfd::SIGINT, &TrapDisposition::Default), "");
}

/// The order is the store's: `EXIT` first, then ascending condition numbers — the
/// one action shared by three conditions prints three lines.
#[test]
fn the_store_prints_in_its_own_order() {
    let mut state = ShellState::new();
    state.set_trap(signalfd::SIGUSR1, TrapDisposition::Action(word("echo x")));
    state.set_trap(signalfd::SIGINT, TrapDisposition::Action(word("echo x")));
    state.set_trap(EXIT, TrapDisposition::Action(word("echo x")));
    let mut out = Vec::new();
    for (cond, disposition) in state.trap_list() {
        list::push_line(cond, disposition, &mut out).unwrap();
    }
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "trap -- 'echo x' EXIT\ntrap -- 'echo x' SIGINT\ntrap -- 'echo x' SIGUSR1\n"
    );
}
