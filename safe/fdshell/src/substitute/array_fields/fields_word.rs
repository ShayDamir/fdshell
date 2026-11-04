//! The whole-word arm of the shell string array reads: the fast path of
//! `substitute/fields.rs`, the twin of `positional::expand_positional_word`.

use alloc::vec;
use alloc::vec::Vec;

use error_stack::Report;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::{MaskBit, region_len};
use crate::state::ShellState;

use super::Shape;
use super::{join, list, shape};
use crate::substitute::borrow_state;
use crate::substitute::origin::Origin;
use crate::substitute::positional::first_ifs_byte;
use crate::substitute::split::split_word;

/// The whole-word shape of the fast path in `substitute/fields.rs`: a word that
/// is exactly `${name[@]}` / `${name[*]}` (the `${ … }` wrapper stripped), the
/// whole word inside the trigger byte's quote region — the `$@` guard's analogue,
/// so a `$` whose region ends at the `$` starts no reference and the general path
/// gets it.
pub(crate) fn whole_word(word: &ShortCStr, mask: &[MaskBit]) -> Option<(ShortCStr, bool)> {
    if region_len(mask, 0) < word.len() {
        return None;
    }
    let content = word.strip_prefix(b"${")?.strip_suffix(b"}")?;
    // The word must be ONE substitution: the first `}` closes the brace (the
    // arm's own `read_until_close`), so a `}` inside the content is a second
    // expansion, and a `#`/`!` content is the length or the indirect arm — the
    // fast path claims only `name[@]` / `name[*]`.
    if content.find_byte(b'}').is_some() || content.starts_with(b"#") || content.starts_with(b"!") {
        return None;
    }
    match shape(&content)? {
        Shape::All { name, star } => Some((name, star)),
        Shape::Element { .. } => None,
    }
}

/// The whole-word arm's fields: the same empty-deque contract as the positional
/// path — fully quoted `[@]` yields no field at all, fully quoted `[*]` its single
/// joined word, empty.
pub(crate) fn fields(
    name: &ShortCStr,
    is_star: bool,
    fq: bool,
    quoted: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<(ShortCStr, Vec<MaskBit>)>, Report<ResolveError>> {
    let state = borrow_state(cell)?;
    let elements = list(name, &state);
    if fq && is_star {
        let word = join(&elements, first_ifs_byte(&state.ifs)?)?;
        return Ok(vec![(word.clone(), vec![MaskBit::Quoted; word.len()])]);
    }
    let mut out = Vec::new();
    for element in &elements {
        if fq {
            out.push((element.clone(), vec![MaskBit::Quoted; element.len()]));
            continue;
        }
        let origin = Origin::all(element.len());
        for (field, mask) in split_word(element, &[], &state.ifs, &origin)? {
            out.push((field, vec![MaskBit::Unquoted; mask.len()]));
        }
    }
    if fq {
        return Ok(out);
    }
    if quoted && out.is_empty() {
        out.push((ShortCStr::new(), Vec::new()));
    }
    Ok(out)
}
