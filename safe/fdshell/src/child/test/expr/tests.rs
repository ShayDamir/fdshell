//! Unit tests for the `test` expression parser and evaluator.

#![allow(clippy::unwrap_used)]

use alloc::boxed::Box;
use alloc::ffi::CString;
use alloc::vec::Vec;
use core::ffi::CStr;

use builtins::error::BuiltinError;
use sys::ShortCStr;

use crate::state::ShellState;

use super::Expr;
use super::eval_expr;
use super::parse::parse;

/// Build substituted (`refs`) and original (`origs`) argument views for `args`
/// and call `f`; the backing strings live for the duration of the call.
fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&CStr], &[ShortCStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    f(&refs, &origs)
}

fn run(args: &[&str], state: &ShellState) -> Result<i32, error_stack::Report<BuiltinError>> {
    with_refs(args, |refs, origs| eval_expr(refs, origs, state))
}

fn parse_ast(args: &[&str]) -> Expr {
    with_refs(args, |refs, _origs| parse(refs).unwrap())
}

#[test]
fn single_argument_rule() {
    let state = ShellState::new();
    // A lone word — even one that looks like an operator — is a string.
    assert_eq!(run(&["x"], &state).unwrap(), 0);
    assert_eq!(run(&["-f"], &state).unwrap(), 0);
    assert_eq!(run(&["!"], &state).unwrap(), 0);
    assert_eq!(run(&[""], &state).unwrap(), 1);
}

#[test]
fn or_truth_table() {
    let state = ShellState::new();
    assert_eq!(
        run(&["1", "-eq", "1", "-o", "1", "-eq", "2"], &state).unwrap(),
        0
    );
    assert_eq!(
        run(&["1", "-eq", "2", "-o", "1", "-eq", "1"], &state).unwrap(),
        0
    );
    assert_eq!(
        run(&["1", "-eq", "2", "-o", "1", "-eq", "2"], &state).unwrap(),
        1
    );
}

#[test]
fn and_truth_table() {
    let state = ShellState::new();
    assert_eq!(
        run(&["1", "-eq", "1", "-a", "2", "-eq", "2"], &state).unwrap(),
        0
    );
    assert_eq!(
        run(&["1", "-eq", "1", "-a", "2", "-eq", "3"], &state).unwrap(),
        1
    );
    assert_eq!(
        run(&["1", "-eq", "2", "-a", "2", "-eq", "3"], &state).unwrap(),
        1
    );
}

#[test]
fn negation() {
    let state = ShellState::new();
    assert_eq!(run(&["!", "1", "-eq", "2"], &state).unwrap(), 0);
    assert_eq!(run(&["!", "1", "-eq", "1"], &state).unwrap(), 1);
    assert_eq!(run(&["!", "!", "1", "-eq", "1"], &state).unwrap(), 0);
    // `!` binds tighter than `-a`/`-o`: `! a -o b` = `(!a) -o b`.
    assert_eq!(
        run(&["!", "1", "-eq", "1", "-o", "1", "-eq", "2"], &state).unwrap(),
        1
    );
    assert_eq!(
        run(&["!", "1", "-eq", "2", "-o", "1", "-eq", "2"], &state).unwrap(),
        0
    );
}

#[test]
fn grouping() {
    let state = ShellState::new();
    assert_eq!(
        run(
            &["(", "1", "-eq", "2", ")", "-o", "(", "1", "-eq", "1", ")"],
            &state
        )
        .unwrap(),
        0
    );
    assert_eq!(
        run(
            &["(", "1", "-eq", "1", ")", "-a", "(", "1", "-eq", "1", ")"],
            &state
        )
        .unwrap(),
        0
    );
    assert_eq!(
        run(
            &["!", "(", "1", "-eq", "2", ")", "-a", "1", "-eq", "1"],
            &state
        )
        .unwrap(),
        0
    );
}

#[test]
fn and_binds_tighter_than_or() {
    let state = ShellState::new();
    // `t -o f -a f` = `t -o (f -a f)` = true; `(t -o f) -a f` would be false.
    assert_eq!(
        run(
            &[
                "1", "-eq", "1", "-o", "1", "-eq", "2", "-a", "1", "-eq", "3"
            ],
            &state
        )
        .unwrap(),
        0
    );
    // Chained operators stay left-associative.
    assert_eq!(
        run(
            &[
                "1", "-eq", "2", "-o", "1", "-eq", "2", "-o", "1", "-eq", "1"
            ],
            &state
        )
        .unwrap(),
        0
    );
    assert_eq!(
        run(
            &[
                "1", "-eq", "2", "-o", "1", "-eq", "2", "-o", "1", "-eq", "2"
            ],
            &state
        )
        .unwrap(),
        1
    );
}

fn assert_non_integer(args: &[&str], state: &ShellState) {
    let report = run(args, state).unwrap_err();
    assert!(
        matches!(report.current_context(), BuiltinError::TestNonInteger),
        "expected TestNonInteger for {args:?}"
    );
}

#[test]
fn both_sides_of_logical_ops_are_evaluated() {
    let state = ShellState::new();
    // No short-circuit: the integer error on the `-o`'s second side is
    // reported even though the first side is already true.
    assert_non_integer(&["2", "-eq", "2", "-o", "1", "-eq", "a"], &state);
    // Same for `-a`: an error from the second operand propagates.
    assert_non_integer(&["1", "-eq", "1", "-a", "x", "-eq", "1"], &state);
    // A non-integer on the first `-a` operand is reported too.
    assert_non_integer(&["x", "-eq", "1", "-a", "1", "-eq", "1"], &state);
    // An error inside a group is reported as well.
    assert_non_integer(&["(", "1", "-eq", "a", ")"], &state);
}

#[test]
fn malformed_expressions() {
    let state = ShellState::new();
    let cases: &[&[&str]] = &[
        &["(", "1", "-eq", "1"], // unclosed group
        &["1", "-o"],            // no right operand
        &["a", "-o", "b", "c"],  // stray trailing token
        &["a", "b", "c", "d"],
        &["a", "b"],
        &["a", "~", "b"],
        &["1", "-o", "-f"], // a unary op with no operand mid-expression
    ];
    for args in cases {
        assert!(
            matches!(
                run(args, &state).unwrap_err().current_context(),
                BuiltinError::TestUsage
            ),
            "expected TestUsage for {args:?}"
        );
    }
}

#[test]
fn precedence_and_shape() {
    // `a -o b -a c` groups as `a -o (b -a c)`: `-a` binds tighter.
    // Token indices: a=0, -o=1, b=2, -a=3, c=4.
    assert_eq!(
        parse_ast(&["a", "-o", "b", "-a", "c"]),
        Expr::Or(
            Box::new(Expr::Single(0)),
            Box::new(Expr::And(
                Box::new(Expr::Single(2)),
                Box::new(Expr::Single(4))
            ))
        )
    );
    // `! a -o b` groups as `(!a) -o b`.
    assert_eq!(
        parse_ast(&["!", "a", "-o", "b"]),
        Expr::Or(
            Box::new(Expr::Not(Box::new(Expr::Single(1)))),
            Box::new(Expr::Single(3))
        )
    );
    // A group is a primary: `( a -o b ) -a c` = `(a -o b) -a c`.
    // Token indices: (=0, a=1, -o=2, b=3, )=4, -a=5, c=6.
    assert_eq!(
        parse_ast(&["(", "a", "-o", "b", ")", "-a", "c"]),
        Expr::And(
            Box::new(Expr::Or(
                Box::new(Expr::Single(1)),
                Box::new(Expr::Single(3))
            )),
            Box::new(Expr::Single(6))
        )
    );
}
