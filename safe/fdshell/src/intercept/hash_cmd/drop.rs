//! `hash -d name…`: drop the named entries from the command lookup table.

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, bail};
use sys::ShortCStr;

/// `hash -d name…` removes each name's entry. bash measures: a name with no
/// entry reports `hash: NAME: not found` and the builtin exits 1, a name that
/// was present is removed silently, and a bare `hash -d` is a getopt
/// diagnostic (`-d: option requires an argument`, rc 1) — fdshell reports its
/// own `CmdError::HashUsage` for that form. The names are the **expanded**
/// fields, like every other `hash` word.
pub(super) fn delete_names(
    names: &[ShortCStr],
    state: &mut ShellState,
) -> Result<i32, Report<CmdError>> {
    if names.is_empty() {
        bail!(CmdError::HashUsage);
    }
    let mut missing = false;
    for name in names {
        if state.hash_table.remove(name).is_none() {
            super::pin::report_not_found(name);
            missing = true;
        }
    }
    Ok(i32::from(missing))
}
