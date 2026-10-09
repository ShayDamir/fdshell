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
    let quoted: Vec<bool> = vec![false; args.len()];
    parse(&args, &mask, &quoted)
}

/// `parse` with an explicit quote mask per word.
fn run_masked(
    words: &[&str],
    mask: &[Vec<bool>],
) -> Result<super::parse::TimeoutConfig, Report<CmdError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    let quoted: Vec<bool> = vec![false; args.len()];
    parse(&args, mask, &quoted)
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

/// POSIX #4.1: the target word is folded, because it never goes through
/// substitution — `timeout 1 e\cho hi` launches the command `echo`, with the folded
/// mask aligned to the folded text (the escaped byte is protected).
#[test]
fn target_word_folds_escape_pairs() {
    let p = run(&["1", "e\\cho", "hi"]).unwrap();
    assert_eq!(p.command.as_bytes().unwrap(), b"echo");
    assert_eq!(p.command_mask, vec![false, true, false, false]);
    assert_eq!(p.args.len(), 1);
    assert_eq!(p.args.first().unwrap().as_bytes().unwrap(), b"hi");
}

/// A quoted target word has `true` on the backslash, so the pair is kept (POSIX
/// #4.2) and the literal name `e\cho` is what the lookup sees.
#[test]
fn quoted_target_word_keeps_the_pair() {
    let p = run_masked(&["1", "e\\cho"], &[vec![false], vec![true; 5]]).unwrap();
    assert_eq!(p.command.as_bytes().unwrap(), b"e\\cho");
    assert_eq!(p.command_mask, vec![true; 5]);
}
