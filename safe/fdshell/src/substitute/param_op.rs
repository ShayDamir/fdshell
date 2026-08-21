use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

/// Splits `${name:opword}` content at the first `:` followed by `-`, `=`, `+`, or `?`.
pub(super) fn split_operator(content: &ShortCStr) -> Option<(ShortCStr, u8, ShortCStr)> {
    let bytes = content.as_bytes().ok()?;
    let mut i = 0;
    loop {
        match bytes.get(i..i + 2) {
            Some([b':', op]) if matches!(op, b'-' | b'=' | b'+' | b'?') => {
                if i == 0 {
                    return None;
                }
                return Some((content.get(0..i)?, *op, content.get(i + 2..)?));
            }
            Some(_) => i += 1,
            None => return None,
        }
    }
}

/// Applies `${name:opword}`: `-`/`=`/`+`/`?` (default when unset or empty,
/// assign, alternate, error). `=` mutates the state.
pub(super) fn apply_param_op(
    name: &ShortCStr,
    op: u8,
    word: &ShortCStr,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    if op == b'=' {
        let mut state = cell
            .borrow_mut()
            .change_context(ResolveError::RefNotFound)?;
        match super::resolve::var_value(name, &state) {
            Some(val) if !val.is_empty() => out.push(val),
            _ => {
                state.strings.insert(
                    name.clone(),
                    sys::ImportedStr::new(word.clone(), sys::Trace::boundary(sys::Origin::Shell)),
                );
                out.push(word);
            }
        }
        return Ok(());
    }
    let state = super::borrow_state(cell)?;
    let val = super::resolve::var_value(name, &state);
    match op {
        b'-' => match val {
            Some(v) if !v.is_empty() => out.push(v),
            _ => out.push(word),
        },
        b'+' => match val {
            Some(v) if !v.is_empty() => out.push(word),
            _ => {}
        },
        b'?' => match val {
            Some(v) if !v.is_empty() => out.push(v),
            _ => {
                let msg = if word.is_empty() {
                    c"parameter null or not set".into()
                } else {
                    word.clone()
                };
                bail!(ResolveError::ParamNullOrNotSet {
                    var: name.clone(),
                    word: msg
                });
            }
        },
        _ => bail!(ResolveError::Never),
    }
    Ok(())
}
