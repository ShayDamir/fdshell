//! The embedded arm of the shell string array reads, called from
//! `substitute/brace.rs`: the twin of `dollar/at_star.rs`.

use error_stack::{Report, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::origin::Origin;

use super::{Shape, element_value, join, list, shape};
use crate::substitute::borrow_state;
use crate::substitute::positional::first_ifs_byte;

/// `[*]` — and `[@]` whose trigger byte is unquoted — join the elements with the
/// first `IFS` byte into one span the caller splits; a quoted `[@]` pushes them
/// adjacent with a word boundary between. A single element is the store's, the
/// scalar answering index 0. `true` = the content was an array shape and is
/// written; `false` leaves it to the ordinary lookup.
pub(crate) fn braced(
    content: &ShortCStr,
    quoted: bool,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    origin: &mut Origin,
) -> Result<bool, Report<ResolveError>> {
    let Some(shape) = shape(content) else {
        return Ok(false);
    };
    let state = borrow_state(cell)?;
    let nounset = state.options & crate::options::NOUNSET != 0;
    match &shape {
        Shape::All { name, star } => {
            let elements = list(name, &state);
            if *star || !quoted {
                out.push(&join(&elements, first_ifs_byte(&state.ifs)?)?);
            } else {
                adjacent(&elements, out, origin);
            }
        }
        // An unresolved element is the nounset bail (bash reports the whole
        // `a[3]`, measured) or nothing at all.
        Shape::Element { name, index } => match element_value(name, *index, &state) {
            Some(value) => out.push(&value),
            None if nounset => {
                bail!(ResolveError::UnboundVariable {
                    var: content.clone()
                });
            }
            None => {}
        },
    }
    Ok(true)
}

/// `${#name[@]}` (the element count) and `${#name[idx]}` (the element's length),
/// read through the same lookups so no arm can disagree with the expansion.
/// `None` is a content this module does not answer — the `#` arm's own nounset,
/// empty-name and zero readings are unchanged.
pub(crate) fn len(
    content: &ShortCStr,
    cell: &ForkCell<ShellState>,
) -> Result<Option<usize>, Report<ResolveError>> {
    let state = borrow_state(cell)?;
    let Some(shape) = shape(content) else {
        return Ok(None);
    };
    Ok(match &shape {
        Shape::All { name, .. } => Some(list(name, &state).len()),
        Shape::Element { name, index } => element_value(name, *index, &state).map(|v| v.len()),
    })
}

/// Push every element adjacent to the text built so far, recording a quoted word
/// boundary at the start of each one after the first — `at_star::adjacent` for
/// the positionals.
pub(crate) fn adjacent(elements: &[ShortCStr], out: &mut ShortCStr, origin: &mut Origin) {
    for (j, element) in elements.iter().enumerate() {
        if j > 0 {
            origin.boundary(out.len(), true);
        }
        out.push(element);
    }
}
