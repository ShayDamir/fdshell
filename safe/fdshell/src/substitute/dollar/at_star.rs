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
//!
//! The `case` **pattern** surface (`Join::Pattern`, task #319) is a different
//! matrix: bash keeps only the **first expanded word** of a pattern word
//! (`execute_case_command`, `es->word->word`), never a list of patterns, and
//! splits nothing. Every row of it is measured in bash 5.3.9 and pinned in
//! `substitute/pattern.rs`, `substitute/pattern/tests.rs` and `tests/case.rs`.
//! LESSONS: "`$@` has two join rules".

use error_stack::Report;
use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::Join;
use crate::substitute::borrow_state;
use crate::substitute::mask::Counting;
use crate::substitute::origin::Origin;
use crate::substitute::positional::{first_ifs_byte, join, positional_join};

pub(super) fn expand(
    peek: &mut Counting<'_>,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    quoted: bool,
    mode: Join,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    let is_star = peek.next() == Some(b'*');
    let state = borrow_state(cell)?;
    if mode == Join::Pattern {
        return pattern_arm(is_star, quoted, &state, out, origin);
    }
    if is_star || !quoted {
        let before = out.len();
        let joined = positional_join(&state.positional, &state.ifs)?;
        out.push(&joined);
        origin.record(before, out.len());
        return Ok(());
    }
    adjacent(&state.positional, out, origin);
    Ok(())
}

/// The pattern arm's matrix (all rows measured in bash 5.3.9, task #319):
///
/// - quoted `"$@"`, **non-empty `IFS`** → the boundary model of the word path:
///   the caller's boundary cut keeps `p1` and drops the rest, and the glue
///   that touches `p1` survives (`set -- a b; case a in "$@")` matches,
///   `case b in "$@")` does not; `IFS=,; set -- "a b" c; case "xa b" in
///   x"$@"y)` matches, so the trailing glue `y` is dropped with `p2`).
/// - quoted `"$@"`, **empty `IFS`** → **space join**, no boundary: nothing is
///   dropped (`IFS=; set -- a b; case "a b" in "$@")`,
///   `IFS=; set -- "a b" c; case "xa b cy" in x"$@"y)` match).
/// - **unquoted `$@`** → **space join** whatever the `IFS` is:
///   `IFS=,; set -- a b; case "pa bq" in p$@q)` matches, and no `IFS=,`-joined
///   text (`pa,bq`) ever does.
/// - `$*`, quoted or not → join with the **first `IFS` byte**, and
///   **concatenate** when the `IFS` is empty: `IFS=,; … case "a,b" in $*)`
///   matches, `IFS=; … case ab in $*)` matches. The word path's
///   space-when-empty separator is the one row the pattern surface must not
///   share.
///
/// The space-join rows emit **no** boundary, so nothing is dropped and the glue
/// survives; a pattern is never IFS-split and never globbed (task #280's mask
/// rule), so "join only where a splitter consumes the text" holds — the
/// pattern's cut is a boundary cut, not an `IFS` split.
fn pattern_arm(
    is_star: bool,
    quoted: bool,
    state: &ShellState,
    out: &mut ShortCStr,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    if is_star {
        return joined(&state.positional, first_ifs_byte(&state.ifs)?, out, origin);
    }
    if quoted && first_ifs_byte(&state.ifs)?.is_some() {
        adjacent(&state.positional, out, origin);
        return Ok(());
    }
    joined(&state.positional, Some(b' '), out, origin)
}

/// Push every positional adjacent to the text built so far, recording a
/// quoted word boundary at the start of each one after the first: the word
/// list is the **caller's** to cut — the splitting path splits it, the pattern
/// path cuts on the boundaries and keeps the first piece.
fn adjacent<'a>(
    positional: impl IntoIterator<Item = &'a ImportedStr>,
    out: &mut ShortCStr,
    origin: &mut Origin,
) {
    for (j, p) in positional.into_iter().enumerate() {
        if j > 0 {
            origin.boundary(out.len(), true);
        }
        let before = out.len();
        out.push(p);
        origin.record(before, out.len());
    }
}

/// Push the positionals joined by `sep` (`None` concatenates) as one
/// expansion span.
fn joined<'a>(
    positional: impl IntoIterator<Item = &'a ImportedStr>,
    sep: Option<u8>,
    out: &mut ShortCStr,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    let before = out.len();
    out.push(&join(positional, sep)?);
    origin.record(before, out.len());
    Ok(())
}
