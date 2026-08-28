use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

use super::parse::parse;
use crate::error::cmd::CmdError;

fn run(words: &[&str]) -> Result<super::parse::TimeoutConfig, Report<CmdError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    let mask: Vec<Vec<bool>> = args.iter().map(|a| vec![false; a.len()]).collect();
    parse(&args, &mask)
}

#[test]
fn missing_seconds() {
    let err = run(&[]).unwrap_err();
    assert!(matches!(
        err.current_context(),
        CmdError::TimeoutMissingSeconds
    ));
}

#[test]
fn missing_command() {
    let err = run(&["5"]).unwrap_err();
    assert!(matches!(
        err.current_context(),
        CmdError::TimeoutMissingCommand
    ));
}

#[test]
fn basic() {
    let p = run(&["5", "sleep", "10"]).unwrap();
    assert_eq!(p.seconds, 5);
    assert_eq!(p.command.as_bytes().unwrap(), b"sleep");
    assert_eq!(p.args.len(), 1);
    assert_eq!(p.args.first().unwrap().as_bytes().unwrap(), b"10");
}

#[test]
fn no_command_args() {
    let p = run(&["5", "true"]).unwrap();
    assert_eq!(p.seconds, 5);
    assert!(p.args.is_empty());
}

#[test]
fn multiple_command_args() {
    let p = run(&["2", "echo", "a", "b", "c"]).unwrap();
    assert_eq!(p.args.len(), 3);
    assert_eq!(p.args.get(1).unwrap().as_bytes().unwrap(), b"b");
}

#[test]
fn zero_seconds_is_valid() {
    let p = run(&["0", "true"]).unwrap();
    assert_eq!(p.seconds, 0);
}

#[test]
fn bad_seconds() {
    let err = run(&["abc", "true"]).unwrap_err();
    assert!(matches!(
        err.current_context(),
        CmdError::TimeoutBadSeconds { .. }
    ));
}

#[test]
fn negative_seconds() {
    let err = run(&["-1", "true"]).unwrap_err();
    assert!(matches!(
        err.current_context(),
        CmdError::TimeoutBadSeconds { .. }
    ));
}
