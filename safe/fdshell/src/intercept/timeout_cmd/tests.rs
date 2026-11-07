use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

use super::parse::parse;
use crate::capture::Capture;
use crate::error::cmd::timeout::TimeoutError;
use crate::mask::MaskBit;
use crate::mask::MaskBit::{Quoted, Unquoted};
use crate::parse::{BuiltinPrefix, CommandLine};
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

fn run(words: &[&str]) -> Result<super::parse::TimeoutConfig, Report<TimeoutError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    let mask: Vec<Vec<MaskBit>> = args.iter().map(|a| vec![Unquoted; a.len()]).collect();
    let quoted: Vec<bool> = vec![false; args.len()];
    parse(&args, &mask, &Vec::new(), &quoted)
}

/// `parse` with an explicit quote mask per word.
fn run_masked(
    words: &[&str],
    mask: &[Vec<MaskBit>],
) -> Result<super::parse::TimeoutConfig, Report<TimeoutError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    let quoted: Vec<bool> = vec![false; args.len()];
    parse(&args, mask, &Vec::new(), &quoted)
}

/// `parse` with an explicit quote mask *and* quoted-span flag per word.
fn run_masked_quoted(
    words: &[&str],
    mask: &[Vec<MaskBit>],
    quoted: &[bool],
) -> Result<super::parse::TimeoutConfig, Report<TimeoutError>> {
    let args: Vec<ShortCStr> = words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect();
    parse(&args, mask, &Vec::new(), quoted)
}

fn make_cmdline(args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        prefix: BuiltinPrefix::None,
        command: c"timeout".into(),
        command_mask: vec![],
        command_span: vec![],
        command_quoted: false,
        env_assigns: vec![],
        args: args_vec,
        args_mask: vec![vec![]; args.len()],
        args_span: vec![vec![]; args.len()],
        args_quoted: vec![false; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

/// `timeout` is an in-process intercept, so a capture (`%>`) is rejected before
/// any command is parsed or launched.
#[test]
fn captures_are_rejected_with_the_timeout_error() {
    let line = b"timeout 1 echo %>";
    let mut cmdline = make_cmdline(&[]);
    cmdline.captures = vec![Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
        cap: None,
        set_at: sys::Position::new(1, 1),
    }];
    let cell = make_cell();
    let err = super::run_timeout(line, &cmdline, &cell).unwrap_err();
    assert!(matches!(
        err.current_context(),
        TimeoutError::CapturesNotSupported { .. }
    ));
}

/// Rule 19: the arm is unreachable to a *script*, not to a unit test.
/// `command_word::expand` calls `cell.borrow_mut()`, and that borrow is
/// **fallible** (`Err(SharedBorrowActive)`) — so a row that holds a shared borrow
/// of the state across the call reaches `CmdError::Never` and pins this arm's own
/// mapping: under `delete match arm CmdError::Never` the answer becomes `Resolve`.
/// No fork happens here (a forked child would call `reset_after_fork`), and the
/// assertion is on the variant, never on the `at …` frame a debug build adds.
#[test]
fn a_held_shared_borrow_answers_the_never_arm() {
    let cell = make_cell();
    let held = cell.borrow().unwrap();
    let cmdline = make_cmdline(&["1", "true"]);
    let err = super::run_timeout(b"timeout 1 true", &cmdline, &cell).unwrap_err();
    assert!(matches!(err.current_context(), TimeoutError::Never));
    drop(held);
}

#[test]
fn missing_seconds() {
    let err = run(&[]).unwrap_err();
    assert!(matches!(
        err.current_context(),
        TimeoutError::TimeoutMissingSeconds
    ));
}

#[test]
fn missing_command() {
    let err = run(&["5"]).unwrap_err();
    assert!(matches!(
        err.current_context(),
        TimeoutError::TimeoutMissingCommand
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
        TimeoutError::TimeoutBadSeconds { .. }
    ));
}

#[test]
fn negative_seconds() {
    let err = run(&["-1", "true"]).unwrap_err();
    assert!(matches!(
        err.current_context(),
        TimeoutError::TimeoutBadSeconds { .. }
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
    assert_eq!(p.command_mask, vec![Unquoted; 5]);
    assert!(!p.command_quoted);
    assert_eq!(p.args.len(), 1);
    assert_eq!(p.args.first().unwrap().as_bytes().unwrap(), b"hi");
}

/// A quoted target word carries `true` on the backslash, so the pair is kept
/// (POSIX #4.2) and the literal name `e\cho` is what the lookup sees; the
/// quoted span flag is the quoted-empty rule for word 0.
#[test]
fn quoted_target_word_keeps_the_pair() {
    let p = run_masked(&["1", "e\\cho"], &[vec![Unquoted], vec![Quoted; 5]]).unwrap();
    assert_eq!(p.command.as_bytes().unwrap(), b"e\\cho");
    assert_eq!(p.command_mask, vec![Quoted; 5]);
    assert!(!p.command_quoted);
}

/// The quoted-span flag rides on the target word: `timeout 1 ""$(true)` has no
/// mask bit that says the word was quoted, only the span flag does.
#[test]
fn quoted_span_flag_rides_on_the_target_word() {
    let p = run_masked_quoted(
        &["1", "$(true)", "hi"],
        &[vec![Unquoted], vec![Unquoted; 7], vec![Unquoted]],
        &[false, true, false],
    )
    .unwrap();
    assert_eq!(p.command.as_bytes().unwrap(), b"$(true)");
    assert!(p.command_quoted);
}
