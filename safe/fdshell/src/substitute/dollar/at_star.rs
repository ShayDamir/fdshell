//! The `$@` / `$*` arms of `dollar_subst` — the spellings inside a larger
//! word (the whole-word `$@`/`$*` goes through `substitute::positional`
//! instead; the two must agree, pinned side by side in `tests/positional.rs`).
//!
//! `$*` always, and `$@` when the trigger byte is unquoted, join the
//! positionals with the first `IFS` byte into one span: bash splits the joined
//! text back into positionals with the same `IFS`, which the splitter does for
//! free — the separator *is* the field boundary
//! (`IFS=:; set -- a b; printf %s x$@` → `[xa][b]`, `IFS=" "; set -- "a b"
//! "c d"; printf %s x$@y` → `[xa][b][c][dy]`). A quoted `$@` has no separator
//! (bash emits none in a quote region — `IFS=,; x"$@"y` over `a "" b` keeps
//! `[xa][][by]` with no comma from us), so it pushes each positional in turn,
//! recording a span per positional and a quoted word boundary at the start of
//! every positional after the first: each positional is glued to the literal
//! bytes that touch it and keeps the field boundaries between them
//! (`IFS=,; set -- "a b" c; printf %s x"$@"y` → `[xa b][cy]`). A quoted
//! positional that ends the word keeps its field even when empty (`x"$@"`
//! over `a ""` → `[xa][]`); the unquoted empty ones vanish in the separator
//! join the same way bash drops them (`x$@` → `[xa]`).

use error_stack::Report;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::borrow_state;
use crate::substitute::mask::Counting;
use crate::substitute::origin::Origin;
use crate::substitute::positional::positional_join;

pub(super) fn expand(
    peek: &mut Counting<'_>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    quoted: bool,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    let is_star = peek.next() == Some(b'*');
    let state = borrow_state(cell)?;
    if is_star || !quoted {
        let before = out.len();
        let joined = positional_join(&state.positional, &state.ifs)?;
        out.push(&joined);
        origin.record(before, out.len());
        return Ok(());
    }
    for (j, p) in state.positional.iter().enumerate() {
        if j > 0 {
            origin.boundary(out.len(), true);
        }
        let before = out.len();
        out.push(p);
        origin.record(before, out.len());
    }
    Ok(())
}
