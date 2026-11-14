//! The colon operator family (`:-` `:=` `:+` `:?`). Every colon operator carries
//! its own word, which supplies the value, so the family is nounset-exempt:
//! `set -u` prints `x` for `${undef:-x}` (bash). It shares the pattern family's
//! `!name` indirection rule through the same lookup file, and keeps the family's
//! own nounset policy (`colon_value` / `colon_target`), so an unresolvable
//! reference bails here too.
//!
//! The word is a **word**: it runs through the ordinary word pipeline before the
//! arm uses it, and only the arms that take it expand it — a `:+` word is not run
//! when the value wins, a `:-` word is not run when the value wins, and under
//! `set -u` an unbound name inside a word that is never run never bails. `:=`
//! stores the expanded scalar; `:?` expands its message **unquoted** even inside
//! a quoted reference. LESSONS: "An operator word is a word"

// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -- -E 'test(/^substitute::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes)'

use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::borrow_state;
use crate::substitute::origin::Origin;

use super::ParamOp;
use super::word::{Word, push};

/// Applies `-`/`+`/`?` to `name` with `word`, nounset-exempt (`colon_value`
/// resolves `!name` and reads the shell strings and the inherited environment;
/// unset is not an error, an unresolvable `!name` is). The value wins over the
/// word for `:-` and `:?`, the word wins for `:+`; each arm expands only the
/// branch it takes, so a word a branch never reaches is neither run nor expanded
/// (and never bails under `set -u`).
pub(super) fn apply(
    name: &ShortCStr,
    op: ParamOp,
    word: &Word,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    // The shared borrow ends at the block: expanding the word can run a command
    // substitution, which borrows the cell itself (LESSONS).
    let val = {
        let state = borrow_state(cell)?;
        state.colon_value(name)?
    };
    match op {
        ParamOp::Default => match val.as_ref() {
            Some(v) if !v.is_empty() => out.push(v),
            _ => push(&word.expand()?, out, origin),
        },
        ParamOp::Alternate => {
            if val.is_some_and(|v| !v.is_empty()) {
                push(&word.expand()?, out, origin);
            }
        }
        _ => {
            if let Some(v) = val.filter(|v| !v.is_empty()) {
                out.push(&v);
            } else {
                let msg = if word.is_absent() {
                    c"parameter null or not set".into()
                } else {
                    word.expand()?.text
                };
                bail!(ResolveError::ParamNullOrNotSet {
                    var: name.clone(),
                    word: msg
                });
            }
        }
    }
    Ok(())
}

/// `:=`: a parameter that holds a non-empty value keeps it (literal, unexpanded);
/// otherwise the word is expanded **once** and the expansion is stored as a shell
/// variable (traced as a shell boundary) and pushed. The write goes to the
/// parameter the name *points at* — `${!v:=q}` writes the target `v` names —
/// resolved through the shared lookup. A positional or special target has no slot
/// to write into, so it bails with bash's `cannot assign in this way` and leaves
/// no write. The mutable borrow is this arm's own, so it never overlaps a shared
/// borrow: the shared borrow ends at the block and the value and target cross it
/// as owned `ShortCStr`s. The word expands before the mutable borrow, because a
/// command substitution inside it borrows the cell too.
pub(super) fn assign(
    name: &ShortCStr,
    word: &Word,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    origin: &mut Origin,
) -> Result<(), Report<ResolveError>> {
    let (target, value) = {
        let state = borrow_state(cell)?;
        (state.colon_target(name)?, state.colon_value(name)?)
    };
    if let Some(val) = value.filter(|v| !v.is_empty()) {
        out.push(&val);
        return Ok(());
    }
    let expanded = word.expand()?;
    let mut state = cell
        .borrow_mut()
        .change_context(ResolveError::RefNotFound)?;
    if state.is_param_name(&target) {
        bail!(ResolveError::CannotAssign { var: target });
    }
    let text = expanded.text.clone();
    state.set_var(
        target,
        sys::ImportedStr::new(text, sys::Trace::boundary(sys::Origin::Shell)),
    );
    push(&expanded, out, origin);
    Ok(())
}
