//! The **shell string array** reads (task #422): the array twin of
//! `substitute/positional.rs` and `substitute/dollar/at_star.rs`. LESSONS
//! "`$@` has two join rules" applies verbatim — the whole-word path
//! (`fields_word`, called from `substitute/fields.rs`) and the embedded arm
//! (`braced_arm`, called from `substitute/brace.rs`) must agree, and both
//! spellings are pinned side by side in `tests/arrays.rs`.
//!
//! The store is `ShellState::str_arrays` (`state/shell_arrays.rs`) and every read
//! here goes through its `elements_of` — the one-namespace rule that puts the
//! scalar at index 0 — so the spellings cannot drift. `[*]` joins the elements
//! with the first `IFS` byte, `[@]` keeps them apart: one protected field per
//! element in a fully quoted word, each element split separately in an unquoted
//! one, and the quoted arm recording a word boundary at the start of every
//! element after the first so the caller's splitter keeps them apart. The empty
//! array keeps the positional contract (task #219): a fully quoted `[@]` yields no
//! field at all, a fully quoted `[*]` one (empty) field. `[@]`/`[*]` never bail
//! under `set -u`; an unresolved `[idx]` does (bash, measured).

use alloc::vec::Vec;

use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::parse::array_ref;
use crate::state::ShellState;

pub(crate) mod braced_arm;
pub(crate) mod fields_word;

pub(crate) use braced_arm::{braced, len};
pub(crate) use fields_word::{fields, whole_word};

/// What a braced content is: the whole-array reads `[@]`/`[*]`, and the single
/// element `[idx]` through the parse's one `[digits]` splitter.
#[cfg_attr(test, derive(Debug, PartialEq))]
pub(crate) enum Shape {
    All { name: ShortCStr, star: bool },
    Element { name: ShortCStr, index: usize },
}

/// The shape of a braced content. `None` is an ordinary name, and every scalar
/// path then stays exactly as it was.
pub(crate) fn shape(content: &ShortCStr) -> Option<Shape> {
    if let Some((name, index)) = array_ref::split_index_ref(content) {
        return Some(Shape::Element { name, index });
    }
    let star = content.ends_with(b"[*]");
    if !star && !content.ends_with(b"[@]") {
        return None;
    }
    let name = content.get(0..content.len() - 3)?;
    (!name.is_empty()).then_some(Shape::All { name, star })
}

/// The elements of a name, through the store's one-namespace rule.
pub(crate) fn list(name: &ShortCStr, state: &ShellState) -> Vec<ShortCStr> {
    state.elements_of(name)
}

/// One element, through the same rule: the store's element, the scalar answering
/// index 0. `None` is unresolved — the caller decides that reading (the
/// expansion's nounset bail, the length's own path).
pub(crate) fn element_value(
    name: &ShortCStr,
    index: usize,
    state: &ShellState,
) -> Option<ShortCStr> {
    state.str_element(name, index).cloned().or_else(|| {
        (index == 0)
            .then(|| state.scalar_value(name))
            .flatten()
            .cloned()
    })
}

/// Join the elements with `sep` (`None` concatenates), as `positional::join` does
/// for the positionals.
pub(crate) fn join(
    elements: &[ShortCStr],
    sep: Option<u8>,
) -> Result<ShortCStr, Report<ResolveError>> {
    let mut out = ShortCStr::new();
    for (j, element) in elements.iter().enumerate() {
        if j > 0
            && let Some(b) = sep
        {
            out.push_byte(b).change_context(ResolveError::NulByte)?;
        }
        out.push(element);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
