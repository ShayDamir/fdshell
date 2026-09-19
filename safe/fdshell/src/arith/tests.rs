#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use alloc::format;
use alloc::vec::Vec;
use core::ffi::CStr;

use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::op::Op;
use crate::error::resolve::ResolveError;
use crate::state::ShellState;

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn eval(cell: &ForkCell<ShellState>, body: &str) -> i64 {
    crate::arith::eval(body.as_bytes(), cell).unwrap()
}

fn eval_err(cell: &ForkCell<ShellState>, body: &str) -> error_stack::Report<ResolveError> {
    match crate::arith::eval(body.as_bytes(), cell) {
        Ok(v) => panic!("expected an error for {body:?}, got {v}"),
        Err(e) => e,
    }
}

fn set_var(cell: &ForkCell<ShellState>, name: &'static CStr, value: &'static CStr) {
    cell.borrow_mut()
        .unwrap()
        .set_var(name.into(), ImportedStr::shell(value.into()));
}

fn var_bytes(cell: &ForkCell<ShellState>, name: &'static CStr) -> Vec<u8> {
    let state = cell.borrow().unwrap();
    let key = ShortCStr::from(name);
    let value = state.strings.get(&key).unwrap().value.clone();
    value.as_bytes().unwrap().to_vec()
}

/// Build a `ShortCStr` from a runtime string (the `&'static` helpers above
/// can't name the generated `a1..aN` depth-chain variables).
fn nm(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

// --- operators: each test yields a distinct value (mutant resistance) ---

#[test]
fn arith_add() {
    assert_eq!(eval(&cell(), "1+2"), 3);
}

#[test]
fn arith_sub() {
    assert_eq!(eval(&cell(), "5-2"), 3);
}

#[test]
fn arith_mul() {
    assert_eq!(eval(&cell(), "3*4"), 12);
}

#[test]
fn arith_div_truncates() {
    assert_eq!(eval(&cell(), "7/2"), 3);
    assert_eq!(eval(&cell(), "7/-2"), -3);
}

#[test]
fn arith_rem() {
    assert_eq!(eval(&cell(), "7%2"), 1);
}

#[test]
fn arith_pow_is_right_associative() {
    // 2**(3**2) = 512, not (2**3)**2 = 64.
    assert_eq!(eval(&cell(), "2**3**2"), 512);
}

#[test]
fn arith_pow_negative_exponent_is_zero() {
    // Documented deviation: bash errors with "exponent less than 0"; v1
    // returns 0 instead (no error variant for it in scope).
    assert_eq!(eval(&cell(), "2**-1"), 0);
    assert_eq!(eval(&cell(), "0**0"), 1);
    assert_eq!(eval(&cell(), "0**5"), 0);
    assert_eq!(eval(&cell(), "2**63"), i64::MIN);
}

#[test]
fn arith_unary_binds_tighter_than_pow() {
    // bash: -2**2 = (-2)**2 = 4, not -(2**2) = -4.
    assert_eq!(eval(&cell(), "-2**2"), 4);
    assert_eq!(eval(&cell(), "-2**3**2"), -512);
    assert_eq!(eval(&cell(), "~2**2"), 9);
    assert_eq!(eval(&cell(), "-5"), -5);
    assert_eq!(eval(&cell(), "--5"), 5);
}

#[test]
fn arith_unary_plus() {
    assert_eq!(eval(&cell(), "+5"), 5);
}

#[test]
fn arith_logical_not() {
    // Logical not: 1 for zero, 0 for anything else (including negatives).
    assert_eq!(eval(&cell(), "!0"), 1);
    assert_eq!(eval(&cell(), "!5"), 0);
    assert_eq!(eval(&cell(), "!-3"), 0);
}

#[test]
fn arith_bit_not() {
    assert_eq!(eval(&cell(), "~0"), -1);
    assert_eq!(eval(&cell(), "~5"), -6);
}

#[test]
fn arith_shift() {
    assert_eq!(eval(&cell(), "1<<10"), 1024);
    assert_eq!(eval(&cell(), "1024>>10"), 1);
    // Shift counts wrap mod 64, like C/bash.
    assert_eq!(eval(&cell(), "1<<64"), 1);
}

#[test]
fn arith_shift_binds_looser_than_add() {
    assert_eq!(eval(&cell(), "1<<2+1"), 8);
}

#[test]
fn arith_less() {
    assert_eq!(eval(&cell(), "1<2"), 1);
    assert_eq!(eval(&cell(), "2<1"), 0);
    // Equal operands: 0 (the `<`→`<=` mutant would give 1).
    assert_eq!(eval(&cell(), "2<2"), 0);
}

#[test]
fn arith_less_eq() {
    assert_eq!(eval(&cell(), "2<=2"), 1);
    assert_eq!(eval(&cell(), "3<=2"), 0);
}

#[test]
fn arith_greater() {
    assert_eq!(eval(&cell(), "2>1"), 1);
    assert_eq!(eval(&cell(), "1>2"), 0);
    // Equal operands: 0 (the `>`→`>=` mutant would give 1).
    assert_eq!(eval(&cell(), "2>2"), 0);
}

#[test]
fn arith_greater_eq() {
    assert_eq!(eval(&cell(), "2>=2"), 1);
    assert_eq!(eval(&cell(), "1>=2"), 0);
}

#[test]
fn arith_eq() {
    assert_eq!(eval(&cell(), "1==1"), 1);
    assert_eq!(eval(&cell(), "1==2"), 0);
}

#[test]
fn arith_ne() {
    assert_eq!(eval(&cell(), "1!=2"), 1);
    assert_eq!(eval(&cell(), "1!=1"), 0);
}

#[test]
fn arith_bit_and() {
    assert_eq!(eval(&cell(), "12&10"), 8);
    assert_eq!(eval(&cell(), "12&3"), 0);
}

#[test]
fn arith_bit_or() {
    assert_eq!(eval(&cell(), "12|10"), 14);
    assert_eq!(eval(&cell(), "12|3"), 15);
}

#[test]
fn arith_bit_xor() {
    assert_eq!(eval(&cell(), "12^10"), 6);
    assert_eq!(eval(&cell(), "12^12"), 0);
}

#[test]
fn arith_andand_is_boolean() {
    // Like C: the result is 0 or 1, never the operand's value.
    assert_eq!(eval(&cell(), "1&&2"), 1);
    assert_eq!(eval(&cell(), "1&&0"), 0);
    assert_eq!(eval(&cell(), "0&&2"), 0);
}

#[test]
fn arith_oror_is_boolean() {
    assert_eq!(eval(&cell(), "0||0"), 0);
    assert_eq!(eval(&cell(), "0||2"), 1);
    assert_eq!(eval(&cell(), "1||2"), 1);
}

#[test]
fn arith_short_circuit_skips_div_zero() {
    assert_eq!(eval(&cell(), "0&&1/0"), 0);
    assert_eq!(eval(&cell(), "1||1/0"), 1);
    // The taken side still evaluates (div-by-zero there is an error).
    assert!(matches!(
        eval_err(&cell(), "1&&1/0").current_context(),
        ResolveError::ArithDivZero
    ));
}

#[test]
fn arith_ternary() {
    assert_eq!(eval(&cell(), "1?2:3"), 2);
    assert_eq!(eval(&cell(), "0?2:3"), 3);
    // Right-associative: 1 ? 0 : (0 ? 1 : 2) = 0, not (1 ? 0 : 0) ? 1 : 2 = 2.
    assert_eq!(eval(&cell(), "1?0:0?1:2"), 0);
    // The non-taken branch is not evaluated.
    assert_eq!(eval(&cell(), "1?5:1/0"), 5);
    assert_eq!(eval(&cell(), "0?1/0:5"), 5);
}

#[test]
fn arith_precedence() {
    assert_eq!(eval(&cell(), "2+3*4"), 14);
    assert_eq!(eval(&cell(), "2*3+4"), 10);
    assert_eq!(eval(&cell(), "1|2&3"), 3);
    // `^` binds tighter than `|`: (1^2)|3 = 3, not 1^(2|3) = 2.
    assert_eq!(eval(&cell(), "1^2|3"), 3);
    // `&` binds tighter than `^`: (1&2)^3 = 3, not 1&(2^3) = 0.
    assert_eq!(eval(&cell(), "1&2^3"), 3);
    assert_eq!(eval(&cell(), "2<3==1"), 1);
    // `!=`/`==` are left-associative: (2!=3)==0 = 1==0 = 0.
    assert_eq!(eval(&cell(), "2!=3==0"), 0);
    assert_eq!(eval(&cell(), "1!=1==0"), 1);
    assert_eq!(eval(&cell(), "-2*3"), -6);
}

#[test]
fn arith_paren_grouping() {
    assert_eq!(eval(&cell(), "(1+2)*3"), 9);
    assert_eq!(eval(&cell(), "((2))"), 2);
}

// --- literals ---

#[test]
fn arith_decimal() {
    assert_eq!(eval(&cell(), "42"), 42);
    assert_eq!(eval(&cell(), "  7  "), 7);
}

#[test]
fn arith_hex() {
    assert_eq!(eval(&cell(), "0xff"), 255);
    assert_eq!(eval(&cell(), "0Xff"), 255);
    // Uppercase A–F digits (each value, so no single digit is untested).
    assert_eq!(eval(&cell(), "0xFF"), 255);
    assert_eq!(eval(&cell(), "0xABCDEF"), 11259375);
    assert_eq!(eval(&cell(), "0xffffffffffffffff"), -1);
}

#[test]
fn arith_octal() {
    assert_eq!(eval(&cell(), "010"), 8);
    assert_eq!(eval(&cell(), "0"), 0);
}

#[test]
fn arith_octal_digit_outside_base_is_error() {
    assert!(matches!(
        eval_err(&cell(), "08").current_context(),
        ResolveError::ArithSyntax
    ));
    assert!(matches!(
        eval_err(&cell(), "0109").current_context(),
        ResolveError::ArithSyntax
    ));
}

#[test]
fn arith_bare_x_is_error() {
    assert!(matches!(
        eval_err(&cell(), "0x").current_context(),
        ResolveError::ArithSyntax
    ));
}

#[test]
fn arith_decimal_overflow_wraps() {
    assert_eq!(eval(&cell(), "9999999999999999999999"), 1864712049423024127);
}

#[test]
fn arith_wrapping_ops() {
    assert_eq!(eval(&cell(), "9223372036854775807+1"), i64::MIN);
    // 9223372036854775808 lexes as i64::MIN; unary minus wraps back.
    assert_eq!(eval(&cell(), "-9223372036854775808"), i64::MIN);
    // MIN / -1 wraps (C semantics), not an error.
    assert_eq!(eval(&cell(), "9223372036854775808/-1"), i64::MIN);
}

#[test]
fn arith_div_by_zero() {
    assert!(matches!(
        eval_err(&cell(), "1/0").current_context(),
        ResolveError::ArithDivZero
    ));
    assert!(matches!(
        eval_err(&cell(), "5%0").current_context(),
        ResolveError::ArithDivZero
    ));
}

#[test]
fn arith_binop_rejects_non_binary_ops() {
    // The parser only puts binary ops into `Bin` nodes; every other op is a
    // defensive `Never`, not a domain error the user can fix.
    for op in [
        Op::AndAnd,
        Op::OrOr,
        Op::Question,
        Op::Colon,
        Op::Bang,
        Op::Tilde,
        Op::Assign,
        Op::AddAssign,
        Op::SubAssign,
        Op::MulAssign,
        Op::DivAssign,
        Op::RemAssign,
        Op::AndAssign,
        Op::OrAssign,
        Op::XorAssign,
        Op::ShlAssign,
        Op::ShrAssign,
    ] {
        let err = super::binop::binop(op, 1, 2).unwrap_err();
        assert!(
            matches!(err.current_context(), ResolveError::Never),
            "{op:?} should be a Never"
        );
    }
}

// --- variables ---

#[test]
fn arith_var_numeric_value() {
    let c = cell();
    set_var(&c, c"x", c"5");
    assert_eq!(eval(&c, "x"), 5);
    assert_eq!(eval(&c, "$x"), 5);
    assert_eq!(eval(&c, "x+1"), 6);
}

#[test]
fn arith_unset_var_is_zero() {
    assert_eq!(eval(&cell(), "nope"), 0);
    assert_eq!(eval(&cell(), "nope+1"), 1);
}

#[test]
fn arith_empty_var_is_zero() {
    let c = cell();
    set_var(&c, c"x", c"");
    assert_eq!(eval(&c, "x+1"), 1);
}

#[test]
fn arith_var_value_is_re_evaluated() {
    let c = cell();
    set_var(&c, c"x", c"1+1");
    assert_eq!(eval(&c, "x"), 2);
    set_var(&c, c"y", c"0x10");
    assert_eq!(eval(&c, "y"), 16);
}

#[test]
fn arith_var_chains() {
    let c = cell();
    set_var(&c, c"a", c"3");
    set_var(&c, c"b", c"a+1");
    assert_eq!(eval(&c, "b"), 4);
}

#[test]
fn arith_env_var_resolves() {
    let c = cell();
    c.borrow_mut()
        .unwrap()
        .environ
        .push((ShortCStr::from(c"EVAR"), ShortCStr::from(c"7")));
    assert_eq!(eval(&c, "EVAR"), 7);
}

#[test]
fn arith_circular_var() {
    let c = cell();
    set_var(&c, c"x", c"x");
    let err = eval_err(&c, "x");
    assert!(matches!(
        err.current_context(),
        ResolveError::ArithCircular { var } if var.eq_bytes(b"x")
    ));
}

#[test]
fn arith_circular_var_chain() {
    let c = cell();
    set_var(&c, c"x", c"y");
    set_var(&c, c"y", c"x");
    assert!(matches!(
        eval_err(&c, "x").current_context(),
        ResolveError::ArithCircular { .. }
    ));
}

#[test]
fn arith_var_chain_at_depth_cap_is_ok() {
    // A non-cyclic chain exactly at the cap (a1→…→a100="5") resolves: the
    // `>`→`==`/`>=` mutants would cut it off at depth 99.
    let c = cell();
    {
        let mut state = c.borrow_mut().unwrap();
        for i in 1..=99 {
            state.set_var(
                nm(&format!("a{i}")),
                ImportedStr::shell(nm(&format!("a{}", i + 1))),
            );
        }
        state.set_var(nm("a100"), ImportedStr::shell(nm("5")));
    }
    assert_eq!(eval(&c, "a1"), 5);
}

#[test]
fn arith_var_chain_over_depth_cap_is_circular() {
    // One past the cap (a1→…→a101="5") is circular: the `+`→`*` mutant
    // would let it resolve.
    let c = cell();
    {
        let mut state = c.borrow_mut().unwrap();
        for i in 1..=100 {
            state.set_var(
                nm(&format!("a{i}")),
                ImportedStr::shell(nm(&format!("a{}", i + 1))),
            );
        }
        state.set_var(nm("a101"), ImportedStr::shell(nm("5")));
    }
    assert!(matches!(
        eval_err(&c, "a1").current_context(),
        ResolveError::ArithCircular { .. }
    ));
}

#[test]
fn arith_unparseable_value_is_not_integer() {
    let c = cell();
    set_var(&c, c"x", c"hello world");
    let err = eval_err(&c, "x");
    assert!(matches!(
        err.current_context(),
        ResolveError::ArithNotInteger { var } if var.eq_bytes(b"x")
    ));
    set_var(&c, c"y", c"1+");
    assert!(matches!(
        eval_err(&c, "y").current_context(),
        ResolveError::ArithNotInteger { .. }
    ));
}

#[test]
fn arith_deep_value_errors_keep_identity() {
    // The value parses, but evaluating it divides by zero — that error
    // must not be relabeled "not an integer".
    let c = cell();
    set_var(&c, c"x", c"1/0");
    assert!(matches!(
        eval_err(&c, "x").current_context(),
        ResolveError::ArithDivZero
    ));
}

#[test]
fn arith_pid() {
    let c = cell();
    let pid = c.borrow().unwrap().shell_pid;
    assert_eq!(eval(&c, "$$"), i64::from(pid.as_raw()));
}

#[test]
fn arith_last_bg() {
    let c = cell();
    assert_eq!(eval(&c, "$!"), 0);
    c.borrow_mut().unwrap().last_bg_pid = Some(sys::Pid::from_raw(4242));
    assert_eq!(eval(&c, "$!"), 4242);
}

// --- assignment ---

#[test]
fn arith_assign_returns_value_and_writes_back() {
    let c = cell();
    assert_eq!(eval(&c, "x=5"), 5);
    assert_eq!(var_bytes(&c, c"x"), b"5");
}

#[test]
fn arith_compound_assigns() {
    let c = cell();
    set_var(&c, c"x", c"7");
    assert_eq!(eval(&c, "x+=3"), 10);
    assert_eq!(var_bytes(&c, c"x"), b"10");
    assert_eq!(eval(&c, "x-=4"), 6);
    assert_eq!(eval(&c, "x*=2"), 12);
    assert_eq!(eval(&c, "x/=5"), 2);
    assert_eq!(eval(&c, "x%=2"), 0);
    set_var(&c, c"x", c"12");
    assert_eq!(eval(&c, "x&=10"), 8);
    // `|=` on an overlapping operand: with bit 0 set, `9|1=9` but the
    // `|`→`^` mutant would give `9^1=8`.
    set_var(&c, c"x", c"9");
    assert_eq!(eval(&c, "x|=1"), 9);
    assert_eq!(eval(&c, "x^=1"), 8);
    assert_eq!(eval(&c, "x<<=1"), 16);
    assert_eq!(eval(&c, "x>>=2"), 4);
}

#[test]
fn arith_assign_chains_right_to_left() {
    let c = cell();
    assert_eq!(eval(&c, "a=b=7"), 7);
    assert_eq!(var_bytes(&c, c"a"), b"7");
    assert_eq!(var_bytes(&c, c"b"), b"7");
}

#[test]
fn arith_assign_unset_compound() {
    let c = cell();
    assert_eq!(eval(&c, "x+=3"), 3);
    assert_eq!(var_bytes(&c, c"x"), b"3");
}

#[test]
fn arith_assign_lhs_must_be_name() {
    assert!(matches!(
        eval_err(&cell(), "1=2").current_context(),
        ResolveError::ArithSyntax
    ));
    assert!(matches!(
        eval_err(&cell(), "(1)=2").current_context(),
        ResolveError::ArithSyntax
    ));
}

#[test]
fn arith_assign_div_by_zero() {
    assert!(matches!(
        eval_err(&cell(), "x/=0").current_context(),
        ResolveError::ArithDivZero
    ));
}

// --- syntax errors ---

#[test]
fn arith_empty_body() {
    assert!(matches!(
        eval_err(&cell(), "").current_context(),
        ResolveError::ArithSyntax
    ));
}

#[test]
fn arith_trailing_garbage() {
    for body in [
        "1+", "1 2", ")", "+", "$", "$(", "1:2", "1?", "1?2", "(1", "1))", "a=b=c d", "@", "1@2",
    ] {
        let err = eval_err(&cell(), body);
        assert!(
            matches!(err.current_context(), ResolveError::ArithSyntax),
            "{body:?} should be a syntax error, got {err:?}"
        );
    }
}

#[test]
fn arith_unsupported_dollar_forms() {
    // v1: `$` before `(`, `)` or a non-name byte is a syntax error.
    for body in ["$1", "$(", "$)", "$-1", "$ x"] {
        let err = eval_err(&cell(), body);
        assert!(
            matches!(err.current_context(), ResolveError::ArithSyntax),
            "{body:?} should be a syntax error"
        );
    }
}

// --- render ---

#[test]
fn arith_render() {
    assert!(crate::arith::render(0).unwrap().eq_bytes(b"0"));
    assert!(crate::arith::render(-42).unwrap().eq_bytes(b"-42"));
    assert!(
        crate::arith::render(i64::MAX)
            .unwrap()
            .eq_bytes(b"9223372036854775807")
    );
    assert!(
        crate::arith::render(i64::MIN)
            .unwrap()
            .eq_bytes(b"-9223372036854775808")
    );
}
