#![allow(clippy::unwrap_used)]
use super::*;
use crate::error::cmd::CmdError;

fn guarded_bool(depth: u32, called: &mut bool) -> Result<u8, Report<CmdError>> {
    guarded(depth, CmdError::NestingTooDeep, || {
        *called = true;
        Ok(7)
    })
}

#[test]
fn under_the_cap_the_closure_runs() {
    let mut called = false;
    assert_eq!(guarded_bool(0, &mut called).unwrap(), 7);
    assert!(called, "under the cap nothing must be checked");
}

#[test]
fn at_the_cap_the_closure_runs() {
    // The boundary: exactly `MAX_DATA_DEPTH` is admitted (the `>`→`>=` mutant
    // would refuse it).
    let mut called = false;
    assert_eq!(guarded_bool(MAX_DATA_DEPTH, &mut called).unwrap(), 7);
    assert!(called);
}

#[test]
fn past_the_cap_the_guard_reports() {
    // One past the cap (the `>`→`==`/`>=` mutant would let it recurse).
    let mut called = false;
    let err = guarded_bool(MAX_DATA_DEPTH + 1, &mut called).unwrap_err();
    assert!(matches!(err.current_context(), CmdError::NestingTooDeep));
    assert!(!called, "the guarded depth must not run the closure");
}

#[test]
fn an_inner_error_keeps_its_identity() {
    let result: Result<u8, Report<CmdError>> = guarded(0, CmdError::NestingTooDeep, || {
        Err(Report::new(CmdError::Invalid))
    });
    assert!(matches!(
        result.unwrap_err().current_context(),
        CmdError::Invalid
    ));
}
