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

/// `parse` with an explicit quote mask *and* quoted-span flag per word.
fn run_masked_quoted(
    words: &[&str],
    mask: &[Vec<bool>],
    quoted: &[bool],
) -> Result<super::parse::TimeoutConfig, Report<CmdError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    parse(&args, mask, quoted)
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

/// The target word is word 0 of the sub-command, so `parse` keeps it raw:
/// `command_word::expand` substitutes, folds, splits and globs it before the
/// launch (task #268) — folding it here would decode the single-quote encoding
/// before the substitution pass ever sees the word. The mask stays aligned to
/// the raw text.
#[test]
fn target_word_is_raw_for_the_word_zero_expansion() {
    let p = run(&["1", "e\\cho", "hi"]).unwrap();
    assert_eq!(p.command.as_bytes().unwrap(), b"e\\cho");
    assert_eq!(p.command_mask, vec![false; 5]);
    assert!(!p.command_quoted);
    assert_eq!(p.args.len(), 1);
    assert_eq!(p.args.first().unwrap().as_bytes().unwrap(), b"hi");
}

/// A quoted target word carries `true` on the backslash, so the pair is kept
/// (POSIX #4.2) and the literal name `e\cho` is what the lookup sees; the
/// quoted span flag is the quoted-empty rule for word 0.
#[test]
fn quoted_target_word_keeps_the_pair() {
    let p = run_masked(&["1", "e\\cho"], &[vec![false], vec![true; 5]]).unwrap();
    assert_eq!(p.command.as_bytes().unwrap(), b"e\\cho");
    assert_eq!(p.command_mask, vec![true; 5]);
    assert!(!p.command_quoted);
}

/// The quoted-span flag rides on the target word: `timeout 1 ""$(true)` has no
/// mask bit that says the word was quoted, only the span flag does.
#[test]
fn quoted_span_flag_rides_on_the_target_word() {
    let p = run_masked_quoted(
        &["1", "$(true)", "hi"],
        &[vec![false], vec![false; 7], vec![false]],
        &[false, true, false],
    )
    .unwrap();
    assert_eq!(p.command.as_bytes().unwrap(), b"$(true)");
    assert!(p.command_quoted);
}
