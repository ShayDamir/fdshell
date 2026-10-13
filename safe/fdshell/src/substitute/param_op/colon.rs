//! The colon operator family (`:-` `:=` `:+` `:?`). Every colon operator carries
//! its own word, which supplies the value, so the family is nounset-exempt:
//! `set -u` prints `x` for `${undef:-x}` (bash). It shares the pattern family's
//! `!name` indirection rule through the same lookup file, and keeps the family's
//! own nounset policy (`colon_value` / `colon_target`), so an unresolvable
//! reference bails here too.

use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::borrow_state;

use super::ParamOp;

/// Applies `-`/`+`/`?` to `name` with `word`, nounset-exempt (`colon_value`
/// resolves `!name` and reads the shell strings and the inherited environment;
/// unset is not an error, an unresolvable `!name` is).
pub(super) fn apply(
    name: &ShortCStr,
    op: ParamOp,
    word: &ShortCStr,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    let state = borrow_state(cell)?;
    let val = state.colon_value(name)?;
    match op {
        ParamOp::Default => out.push(match val.as_ref() {
            Some(v) if !v.is_empty() => v,
            _ => word,
        }),
        ParamOp::Alternate => {
            if val.is_some_and(|v| !v.is_empty()) {
                out.push(word);
            }
        }
        _ => {
            if let Some(v) = val.filter(|v| !v.is_empty()) {
                out.push(&v);
            } else {
                let msg = if word.is_empty() {
                    c"parameter null or not set".into()
                } else {
                    word.clone()
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

/// `:=`: a parameter that holds a non-empty value keeps it; otherwise the word
/// is stored as a shell variable (traced as a shell boundary) and pushed. The
/// write goes to the parameter the name *points at* — `${!v:=q}` writes the
/// target `v` names — resolved through the shared lookup. A positional or
/// special target has no slot to write into, so it bails with bash's
/// `cannot assign in this way` and leaves no write. The mutable borrow is this
/// arm's own, so it never overlaps a shared borrow: the shared borrow ends at
/// the block and the value and target cross it as owned `ShortCStr`s.
pub(super) fn assign(
    name: &ShortCStr,
    word: &ShortCStr,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    let (target, value) = {
        let state = borrow_state(cell)?;
        (state.colon_target(name)?, state.colon_value(name)?)
    };
    if let Some(val) = value.filter(|v| !v.is_empty()) {
        out.push(&val);
        return Ok(());
    }
    let mut state = cell
        .borrow_mut()
        .change_context(ResolveError::RefNotFound)?;
    if state.is_param_name(&target) {
        bail!(ResolveError::CannotAssign { var: target });
    }
    state.set_var(
        target,
        sys::ImportedStr::new(word.clone(), sys::Trace::boundary(sys::Origin::Shell)),
    );
    out.push(word);
    Ok(())
}
