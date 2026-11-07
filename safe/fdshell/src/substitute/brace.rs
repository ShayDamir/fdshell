use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;
use crate::substitute::Join;
use crate::substitute::dollar::at_star::pattern_arm;

mod length;

// The `join` mode lets the braced `${@}`/`${*}` reach the shared pattern arm;
// threading it through `substitute/resolve.rs` instead would put the argument on
// every braced surface, arithmetic and nounset included (task #452).
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_brace(
    peek: &mut super::mask::Counting<'_>,
    mask: &[MaskBit],
    start: usize,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
    quoted: bool,
    join: Join,
    origin: &mut super::origin::Origin,
) -> Result<(), Report<ResolveError>> {
    peek.next();
    if peek.peek().copied() == Some(b'#') {
        return length::length(peek, cell, out);
    }
    let (content, closed) = read_until_close(peek)?;
    if !closed {
        out.push(c"${");
        out.push(&content);
        return Ok(());
    }
    // Degenerate braced names never reach a param-op dispatch, so a bail never
    // sees an empty name it cannot print (STYLE §4.7): `${}` and `${!}` (an
    // empty indirect name) stay literal — bash rejects `${}` (`bad
    // substitution`, rc 1) and expands `${!}` to empty at rc 0 — and the
    // operator scan's empty name (`${%x#y}`: a pattern operator at index 0)
    // reads the content as the parameter name, as master does.
    let indirect = content.strip_prefix(b"!");
    if content.is_empty() || indirect.as_ref().is_some_and(|name| name.is_empty()) {
        out.push(c"${");
        out.push(&content);
        out.push(c"}");
        return Ok(());
    }
    match super::param_op::split_operator(&content) {
        // `start` is the word index of the `{` byte, so the braced content
        // begins at `start + 1` and the pattern arm can slice its mask.
        Some((name, op, word)) if !name.is_empty() => {
            return super::param_op::apply_param_op(&name, op, &word, mask, start + 1, cell, out);
        }
        _ => {}
    }
    let state = super::borrow_state(cell)?;
    if indirect.is_some() {
        return state.resolve_indirect(&content, out);
    }
    if super::array_fields::braced(&content, quoted, cell, out, origin)? {
        // `${name[@]}`, `${name[*]}`, `${name[idx]}` — the arm twin of
        // `dollar/at_star.rs`, and it must agree with the whole-word fast path
        // in `substitute/fields.rs` (LESSONS "`$@` has two join rules").
        return Ok(());
    }
    // The braced spellings `${@}` / `${*}` of the `$@`/`$*` arm: on the `case`
    // **pattern** surface bash's braced spelling equals the unbraced one (task
    // #452), so these two exact contents go to the shared arm. Every other
    // content, and every other surface, keeps the resolve path — the word and
    // argument surfaces stay on task #177's D1 join (`tests/positional.rs`).
    if join == Join::Pattern {
        let is_star = content.eq_bytes(b"*");
        if is_star || content.eq_bytes(b"@") {
            return pattern_arm(is_star, quoted, &state, out, origin);
        }
    }
    state.resolve_var_name(&content, out)
}

pub(super) fn read_until_close(
    peek: &mut super::mask::Counting<'_>,
) -> Result<(ShortCStr, bool), Report<ResolveError>> {
    let mut name = ShortCStr::new();
    let mut closed = false;
    for nc in peek.by_ref() {
        if nc == b'}' {
            closed = true;
            break;
        }
        name.push_byte(nc).change_context(ResolveError::NulByte)?;
    }
    Ok((name, closed))
}
