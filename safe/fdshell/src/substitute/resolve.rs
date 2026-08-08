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
            out.push_str(val).change_context(ResolveError::Never)?;
        }
        None => {
            out.push(b'$').change_context(ResolveError::Never)?;
            out.push_str(name).change_context(ResolveError::Never)?;
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
    let mut num = ShortCStr::new();
    num.push(first_digit).change_context(ResolveError::Never)?;
    while let Some(&nc) = peek.peek() {
        if nc.is_ascii_digit() {
            num.push(nc).change_context(ResolveError::Never)?;
            peek.next();
        } else {
            break;
        }
    }
    let idx: usize = num.parse().change_context(ResolveError::TooLarge)?;
    if let Some(pos) = state.positional.get(idx) {
        out.push_str(pos).change_context(ResolveError::Never)?;
    }
    Ok(())
}
