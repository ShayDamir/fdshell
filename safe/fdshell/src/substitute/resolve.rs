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
            out.push(val);
        }
        None => match state.environ.iter().find(|(k, _)| k == name) {
            Some((_, val)) => out.push(val),
            None => {
                out.push(c"$");
                out.push(name);
            }
        },
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
    num.push_byte(first_digit)
        .change_context(ResolveError::Never)?;
    while let Some(&nc) = peek.peek() {
        if nc.is_ascii_digit() {
            num.push_byte(nc).change_context(ResolveError::Never)?;
            peek.next();
        } else {
            break;
        }
    }
    let idx: usize = num.parse().change_context(ResolveError::TooLarge)?;
    if let Some(pos) = state.positional.get(idx) {
        out.push(pos);
    }
    Ok(())
}
