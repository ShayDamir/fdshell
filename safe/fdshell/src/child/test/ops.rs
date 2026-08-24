//! Expression operators of `test`.

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

pub(super) fn is_unary(op: &[u8]) -> bool {
    matches!(op, b"-e" | b"-f" | b"-d" | b"-z" | b"-n")
}

/// String tests `-z` (empty) and `-n` (non-empty) on the substituted value.
pub(super) fn string_test(op: &[u8], arg: &CStr) -> Result<i32, Report<BuiltinError>> {
    let empty = arg.to_bytes().is_empty();
    let ok = match op {
        b"-z" => empty,
        b"-n" => !empty,
        // `is_unary` restricts `op`; `-z`/`-n` are the only string tests.
        _ => bail!(BuiltinError::Never),
    };
    Ok(usize::from(!ok) as i32)
}

pub(super) fn file_test(
    op: &[u8],
    arg: &CStr,
    orig: Option<&ShortCStr>,
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let ok = match super::stat::stat_operand(arg, orig, state)? {
        Some(st) => match op {
            b"-e" => true,
            b"-d" => st.mode & sys::stat::S_IFMT == sys::stat::S_IFDIR,
            b"-f" => st.mode & sys::stat::S_IFMT == sys::stat::S_IFREG,
            // `is_unary` restricts `op` to -e/-f/-d.
            _ => bail!(BuiltinError::Never),
        },
        None => false,
    };
    Ok(usize::from(!ok) as i32)
}

pub(super) fn string_or_int_test(
    lhs: &CStr,
    op: &CStr,
    rhs: &CStr,
) -> Result<i32, Report<BuiltinError>> {
    let op = op.to_bytes();
    let result = if op == b"=" || op == b"!=" {
        let eq = lhs.to_bytes() == rhs.to_bytes();
        if op == b"=" { eq } else { !eq }
    } else if is_int_op(op) {
        compare(integer(lhs)?, op, integer(rhs)?)
    } else {
        bail!(BuiltinError::TestUsage);
    };
    Ok(usize::from(!result) as i32)
}

fn is_int_op(op: &[u8]) -> bool {
    matches!(op, b"-eq" | b"-ne" | b"-lt" | b"-le" | b"-gt" | b"-ge")
}

fn integer(v: &CStr) -> Result<i64, Report<BuiltinError>> {
    let mut s = ShortCStr::new();
    s.push(v);
    s.parse::<i64>()
        .change_context(BuiltinError::TestNonInteger)
}

fn compare(l: i64, op: &[u8], r: i64) -> bool {
    match op {
        b"-eq" => l == r,
        b"-ne" => l != r,
        b"-lt" => l < r,
        b"-le" => l <= r,
        b"-gt" => l > r,
        b"-ge" => l >= r,
        _ => false,
    }
}
