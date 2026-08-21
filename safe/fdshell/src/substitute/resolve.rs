use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use sys::ShortCStr;

/// Value of `name` in the shell's strings, then the inherited environment.
pub(super) fn var_value<'a>(name: &'a ShortCStr, state: &'a ShellState) -> Option<&'a ShortCStr> {
    state.strings.get(name).map(|v| &v.value).or_else(|| {
        state
            .environ
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
    })
}

/// Push an unresolved `${name}` — or `${!name}` — as literal text.
pub(super) fn literal_braced(bang: bool, name: &ShortCStr, out: &mut ShortCStr) {
    out.push(c"${");
    if bang {
        out.push(c"!");
    }
    out.push(name);
    out.push(c"}");
}

/// Indirect reference: expand `name`, then expand its value as a variable name.
pub(super) fn resolve_indirect(name: &ShortCStr, state: &ShellState, out: &mut ShortCStr) {
    match var_value(name, state) {
        Some(target) => match var_value(target, state) {
            Some(val) => out.push(val),
            None => literal_braced(false, target, out),
        },
        None => literal_braced(true, name, out),
    }
}

pub(super) fn resolve_var_name(
    name: &ShortCStr,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    match var_value(name, state) {
        Some(val) => out.push(val),
        None => {
            out.push(c"$");
            out.push(name);
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
