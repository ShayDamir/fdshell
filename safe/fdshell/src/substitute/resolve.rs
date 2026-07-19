use alloc::vec;
use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use sys::ShortCStr;

pub(super) fn resolve_var_name(
    name: &ShortCStr,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    match state.strings.get(name) {
        Some(val) => {
            out.extend_from_slice(val.as_bytes().change_context(ResolveError::RefNotFound)?)
                .change_context(ResolveError::NulByte)?;
        }
        None => {
            out.push(b'$').change_context(ResolveError::Never)?;
            out.extend_from_slice(name.as_bytes().change_context(ResolveError::RefNotFound)?)
                .change_context(ResolveError::NulByte)?;
        }
    }
    Ok(())
}

pub(super) fn resolve_positional_index(
    first_digit: u8,
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    let mut num_bytes = vec![first_digit];
    while let Some(&nc) = peek.peek() {
        if nc.is_ascii_digit() {
            num_bytes.push(nc);
            peek.next();
        } else {
            break;
        }
    }
    let num_short = ShortCStr::from_vec(num_bytes).change_context(ResolveError::Never)?;
    let idx: usize = num_short
        .parse()
        .change_context(ResolveError::MalformedRef)?;
    if let Some(pos) = state.positional.get(idx) {
        out.extend_from_slice(pos.as_bytes().change_context(ResolveError::RefNotFound)?)
            .change_context(ResolveError::NulByte)?;
    }
    Ok(())
}
