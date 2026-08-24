use crate::error::cmd::CmdError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

/// Process one `alias` argument: `name=value` defines an alias, a bare name
/// appends its definition to `out`.
pub(super) fn apply_alias_arg(
    state: &mut ShellState,
    arg: &ShortCStr,
    out: &mut Vec<u8>,
) -> Result<(), Report<CmdError>> {
    match arg
        .as_bytes()
        .ok()
        .and_then(|b| b.iter().position(|&c| c == b'='))
    {
        Some(i) => {
            let name = arg.get(..i).ok_or(CmdError::Never)?;
            let value = arg.get(i + 1..).ok_or(CmdError::Never)?;
            state.aliases.insert(name.clone(), value.clone());
        }
        None => display_alias(state, arg, out)?,
    }
    Ok(())
}

/// Append the definition of `name` to `out`, erroring when it is unset.
pub(super) fn display_alias(
    state: &ShellState,
    name: &ShortCStr,
    out: &mut Vec<u8>,
) -> Result<(), Report<CmdError>> {
    let value = state
        .aliases
        .get(name)
        .ok_or(CmdError::AliasNotFound { name: name.clone() })?;
    super::push_definition(name, value, out);
    Ok(())
}
