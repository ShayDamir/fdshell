#![allow(clippy::unwrap_used)]

use super::*;
use crate::error::redirect::OpenRedirectError;
use sys::ShortCStr;

fn new_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn status(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow().unwrap().last_status.exit_code()
}

/// Every classification arm answers true (POSIX XCU 2.5); a non-redirect frame
/// answers false and stays on the abort channel (house rule 19: each arm has a
/// row that fails under its mutation).
#[test]
fn the_classification_arms_answer_true_and_the_non_redirect_arms_answer_false() {
    assert!(classifies(&Report::new(CmdError::Redirect)));
    let ambiguous = Report::new(CmdError::AmbiguousRedirect {
        name: ShortCStr::from(c"{a,b}"),
    });
    assert!(classifies(&ambiguous));
    // The report as the producers build it: the redirect frame rides on top of
    // the leaf context (`change_context` at the producer).
    let nested = Report::new(OpenRedirectError::Open).change_context(CmdError::Redirect);
    assert!(classifies(&nested));
    assert!(!classifies(&Report::new(CmdError::Parse)));
    assert!(!classifies(&Report::new(CmdError::Cd)));
}

/// The launch-path predicate: its arm answers true, every other launch frame
/// answers false (the match runs before the simple-command path re-contexts).
#[test]
fn the_launch_arm_answers_true_and_the_others_answer_false() {
    assert!(classifies_launch(&Report::new(LaunchError::Redirect)));
    assert!(!classifies_launch(&Report::new(LaunchError::Fork)));
    assert!(!classifies_launch(&Report::new(LaunchError::CaptureSocket)));
}

/// One row per gate arm (the POSIX #2.14 list plus `umask`), plus non-members.
#[test]
fn the_gate_names_the_special_builtins() {
    let gated: &[&[u8]] = &[
        b":",
        b".",
        b"eval",
        b"exec",
        b"export",
        b"readonly",
        b"set",
        b"shift",
        b"times",
        b"trap",
        b"unset",
        b"break",
        b"continue",
        b"return",
        b"umask",
    ];
    for name in gated {
        assert!(is_special(name), "{name:?} is on the gate");
    }
    let plain: &[&[u8]] = &[b"echo", b"cd", b"help", b"quit", b"cat", b""];
    for name in plain {
        assert!(!is_special(name), "{name:?} is not on the gate");
    }
}

/// The function-call frame is not a special builtin even when the interception
/// gate would name the lookup's word; a gated name on a non-function call is.
#[test]
fn the_composition_keeps_the_frame_outside_the_gate() {
    let report = Report::new(CmdError::Redirect);
    assert!(converts(b"set", true, &report));
    assert!(converts(b"echo", false, &report));
    assert!(!converts(b"set", false, &report));
    assert!(!converts(b"echo", false, &Report::new(CmdError::Cd)));
}

/// The conversion: the status channel carries 1 (not the errno) and the caller
/// is told to continue.
#[test]
fn the_conversion_sets_the_status_to_one() {
    let cell = new_cell();
    fail(&Report::new(CmdError::Redirect), &cell).unwrap();
    assert_eq!(status(&cell), 1);

    let fresh = new_cell();
    fail(&Report::new(LaunchError::Redirect), &fresh).unwrap();
    assert_eq!(status(&fresh), 1);
}

/// `convert`: a classifying report converts to `Ok(())` plus status 1; a
/// non-redirect report propagates unchanged (the abort scope).
#[test]
fn convert_converts_only_classifying_reports() {
    let cell = new_cell();
    convert(Report::new(CmdError::Redirect), &cell).unwrap();
    assert_eq!(status(&cell), 1);

    let fresh = new_cell();
    let propagated = convert(Report::new(CmdError::Cd), &fresh).unwrap_err();
    assert!(matches!(propagated.current_context(), CmdError::Cd));
    assert_eq!(status(&fresh), 0);
}

/// `pass`: `Ok` rides through, a classifying report converts to `Ok(None)` plus
/// status 1, anything else propagates.
#[test]
fn pass_routes_the_three_result_shapes() {
    let cell = new_cell();
    assert_eq!(pass(Ok(7u8), &cell).unwrap(), Some(7));

    let fresh = new_cell();
    let converted: Option<()> = pass(
        Err(Report::new(CmdError::AmbiguousRedirect {
            name: ShortCStr::from(c"{a,b}"),
        })),
        &fresh,
    )
    .unwrap();
    assert_eq!(converted, None);
    assert_eq!(status(&fresh), 1);

    let cell = new_cell();
    let propagated = pass::<()>(Err(Report::new(CmdError::Cd)), &cell).unwrap_err();
    assert!(matches!(propagated.current_context(), CmdError::Cd));
    assert_eq!(status(&cell), 0);
}
