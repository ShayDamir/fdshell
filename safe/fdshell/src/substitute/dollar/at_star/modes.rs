//! The argument/word surface's arm of `at_star.rs` (task #222), split out for
//! the line budget; the pattern matrix stays with the arm that answers it.
//! The two must keep agreeing on the join rules (LESSONS: "`$@` has two join
//! rules"), so the shared helpers (`adjacent`, `joined`) stay in `at_star.rs`.

// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -- -E 'test(/^substitute::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes)'

use error_stack::Report;
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::origin::Origin;
use crate::substitute::positional::positional_join;

/// A joined shape when `$*` or an unquoted `$@`, the boundary model for a quoted
/// `"$@"`.
pub(super) fn args_arm(
    is_star: bool,
    quoted: bool,
    state: &ShellState,
    out: &mut ShortCStr,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    if is_star || !quoted {
        let before = out.len();
        let joined = positional_join(&state.positional, &state.ifs)?;
        out.push(&joined);
        origin.record(before, out.len());
        return Ok(());
    }
    super::adjacent(&state.positional, out, origin);
    Ok(())
}
