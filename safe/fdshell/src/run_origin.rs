use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{Origin, ShortCStr};

use crate::error::cmd::CmdError;

/// Origin of an assigned value: transitive for a lone variable or positional
/// reference, command output when a substitution ran, otherwise the source
/// line's origin.
pub(crate) fn assign_origin(
    value: &ShortCStr,
    line_origin: Origin,
    cell: &ForkCell<ShellState>,
) -> Result<Origin, Report<CmdError>> {
    let bytes = value.as_bytes().change_context(CmdError::Resolve)?;
    if has_cmd_subst(bytes) {
        return Ok(Origin::CommandOutput);
    }
    if let Some(rest) = bytes.strip_prefix(b"$") {
        if rest == b"?" {
            return Ok(Origin::Shell);
        }
        if let Some(name) = single_var_name(rest) {
            let state = cell.borrow().change_context(CmdError::Never)?;
            if let Some(val) = state.strings.get(&name) {
                return Ok(val.trace.origin.clone());
            }
            return Ok(line_origin);
        }
        if let Some(idx) = parse_index(rest) {
            let state = cell.borrow().change_context(CmdError::Never)?;
            if let Some(pos) = state.positional.get(idx) {
                return Ok(pos.trace.origin.clone());
            }
        }
        return Ok(line_origin);
    }
    if bytes.first() == Some(&b'~') {
        return Ok(Origin::EnvVar(ShortCStr::from(c"HOME")));
    }
    Ok(line_origin)
}

fn has_cmd_subst(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|w| w == b"$(") || bytes.contains(&b'`')
}

/// `rest` is exactly a `NAME` or `{NAME}` variable name.
fn single_var_name(rest: &[u8]) -> Option<ShortCStr> {
    let name = if rest.first() == Some(&b'{') {
        if rest.len() < 3 || rest.last() != Some(&b'}') {
            return None;
        }
        rest.get(1..rest.len() - 1)?
    } else {
        rest
    };
    is_var_name(name).then(|| ShortCStr::from_vec(name.to_vec()).ok().unwrap_or_default())
}

fn is_var_name(name: &[u8]) -> bool {
    let mut chars = name.iter();
    matches!(chars.next(), Some(&c) if c.is_ascii_alphabetic() || c == b'_')
        && chars.all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

/// `rest` is a positional index (`1`, `12`, …); `$` already stripped.
fn parse_index(rest: &[u8]) -> Option<usize> {
    if rest.is_empty() || !rest.iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.iter().try_fold(0usize, |acc, &d| {
        acc.checked_mul(10)
            .and_then(|a| a.checked_add((d - b'0') as usize))
    })
}

#[cfg(test)]
mod tests;
