use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;
use sys::signalfd::{SFD_NONBLOCK, SIGINT, SIGTERM, SIGUSR1};

use super::parse::{Parsed, parse};
use crate::error::cmd::CmdError;

fn run(words: &[&str]) -> Result<Parsed, Report<CmdError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    parse(&args)
}

#[test]
fn single_signal_by_name() {
    let p = run(&["%s", "INT"]).unwrap();
    assert_eq!(p.var.as_bytes().unwrap(), b"%s");
    assert_eq!(p.signals, vec![SIGINT]);
    assert_eq!(p.flags, 0);
}

#[test]
fn multiple_signals() {
    let p = run(&["%s", "INT", "TERM"]).unwrap();
    assert_eq!(p.signals, vec![SIGINT, SIGTERM]);
}

#[test]
fn signal_by_number() {
    let p = run(&["%s", "2"]).unwrap();
    assert_eq!(p.signals, vec![2]);
}

#[test]
fn flags_nonblock() {
    let p = run(&["%s", "INT", "--flags", "SFD_NONBLOCK"]).unwrap();
    assert_eq!(p.flags, SFD_NONBLOCK);
}

#[test]
fn flags_hex() {
    let p = run(&["%s", "INT", "--flags", "0x800"]).unwrap();
    assert_eq!(p.flags, SFD_NONBLOCK);
}

#[test]
fn flags_then_more_signals() {
    // `--flags` at index 3: `i += 2` lands on the following signal, `i *= 2`
    // would skip it. A mis-stepped index would drop USR1.
    let p = run(&["%s", "INT", "TERM", "--flags", "SFD_NONBLOCK", "USR1"]).unwrap();
    assert_eq!(p.signals, [SIGINT, SIGTERM, SIGUSR1]);
    assert_eq!(p.flags, SFD_NONBLOCK);
}

#[test]
fn missing_var_is_error() {
    let r = run(&[]);
    assert!(
        matches!(r.unwrap_err().current_context(), CmdError::SignalfdNoVar),
        "no args must be a missing-var error"
    );
}

#[test]
fn var_must_start_with_percent() {
    let r = run(&["s", "INT"]);
    assert!(
        matches!(r.unwrap_err().current_context(), CmdError::SignalfdNoVar),
        "a var without % must be a missing-var error"
    );
}

#[test]
fn no_signals_is_error() {
    let r = run(&["%s"]);
    assert!(
        matches!(r.unwrap_err().current_context(), CmdError::SignalfdNoVar),
        "a var with no signals must be an error"
    );
}

#[test]
fn bad_signal_is_error() {
    let r = run(&["%s", "FOO"]);
    assert!(
        matches!(
            r.unwrap_err().current_context(),
            CmdError::SignalfdBadSignal { .. }
        ),
        "an unknown signal name must be rejected"
    );
}

#[test]
fn unknown_flag_is_error() {
    let r = run(&["%s", "-x"]);
    assert!(
        matches!(
            r.unwrap_err().current_context(),
            CmdError::SignalfdBadFlag { .. }
        ),
        "an unknown flag must be rejected"
    );
}

#[test]
fn bad_flag_value_is_error() {
    let r = run(&["%s", "INT", "--flags", "NOPE"]);
    assert!(
        matches!(
            r.unwrap_err().current_context(),
            CmdError::SignalfdBadFlag { .. }
        ),
        "an unknown flag value must be rejected"
    );
}

#[test]
fn usr1_resolves() {
    let p = run(&["%s", "USR1"]).unwrap();
    assert_eq!(p.signals, vec![SIGUSR1]);
}

#[test]
fn all_signal_names_resolve() {
    let names = [
        "HUP", "INT", "QUIT", "ILL", "TRAP", "ABRT", "BUS", "FPE", "SEGV", "PIPE", "ALRM", "TERM",
        "CHLD", "CONT", "STOP", "TSTP", "TTIN", "TTOU", "URG", "XCPU", "XFSZ", "VTALRM", "PROF",
        "WINCH", "IO", "PWR", "SYS", "USR1", "USR2",
    ];
    for name in &names {
        let p = run(&["%s", name]).unwrap();
        assert!(!p.signals.is_empty(), "{name} must resolve to a signal");
    }
}
