//! `-fdsize+N` / `-fdsize-N` — fdshell-only `test` operators comparing an
//! operand's size with a bound fused into the operator word: `+N` is true
//! iff the size is at least `N` bytes, `-N` iff at most `N`.

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::Report;
use sys::ShortCStr;

use super::stat::stat_operand;

/// The size bound fused into a `-fdsize±N` operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fdsize {
    AtLeast(u64),
    AtMost(u64),
}

/// Recognize `-fdsize+<N>` / `-fdsize-<N>`: a sign, then one or more digits
/// fitting in `u64`. `None` for everything else (`-fdsize` alone, no digits,
/// no sign, non-digits, overflow) so the caller falls through to `TestUsage`.
pub(super) fn parse(op: &[u8]) -> Option<Fdsize> {
    let rest = op.strip_prefix(b"-fdsize")?;
    let (at_least, digits) = match rest.first() {
        Some(b'+') => (true, rest.get(1..)?),
        Some(b'-') => (false, rest.get(1..)?),
        _ => return None,
    };
    // Strict digits: `u64::from_str` would also accept a leading `+`/`-`.
    if digits.is_empty() || !digits.iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut n = ShortCStr::new();
    n.push_checked(digits).ok()?;
    let bound = n.parse::<u64>().ok()?;
    Some(if at_least {
        Fdsize::AtLeast(bound)
    } else {
        Fdsize::AtMost(bound)
    })
}

/// Evaluate the operator on `arg` (a path or a `%var` fd var); an unset var
/// or a missing path is false, not an error.
pub(super) fn test(
    spec: Fdsize,
    arg: &CStr,
    orig: Option<&ShortCStr>,
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let Some(st) = stat_operand(arg, orig, state, true)? else {
        return Ok(1);
    };
    let ok = match spec {
        Fdsize::AtLeast(n) => st.size >= n,
        Fdsize::AtMost(n) => st.size <= n,
    };
    Ok(usize::from(!ok) as i32)
}

#[cfg(test)]
mod tests;
