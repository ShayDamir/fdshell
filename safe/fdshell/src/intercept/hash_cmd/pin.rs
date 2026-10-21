//! `hash`'s pin/lookup path: `hash name` prints the entry (PATH-searching and
//! storing on a miss); `hash name path` pins the entry.

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// `hash name` prints the entry (PATH-searching and storing on a miss);
/// `hash name path` pins the entry (`path` = `operands.first()`). `name` is
/// the **expanded** word — the caller ran the argument pipeline, which is also
/// the key shape `prehash` stores for word 0. A third field is fdshell's
/// usage error (bash hashes each name; the listing surface is task #375).
pub(super) fn lookup_or_pin(
    name: &ShortCStr,
    operands: &[ShortCStr],
    state: &mut ShellState,
) -> Result<i32, Report<CmdError>> {
    match operands.first() {
        Some(path) => {
            if operands.get(1).is_some() {
                bail!(CmdError::HashUsage);
            }
            state.hash_table.insert(name.clone(), path.clone());
            Ok(0)
        }
        None => lookup(name, state),
    }
}

/// `hash name`: print the entry, PATH-searching and caching it on a miss.
fn lookup(name: &ShortCStr, state: &mut ShellState) -> Result<i32, Report<CmdError>> {
    let path = state.hash_table.get(name).cloned().or_else(|| {
        crate::exec::resolve_path_str(name, &state.hash_table, state.strict(), state.cwd_fd()).ok()
    });
    match path {
        Some(p) => {
            state.hash_table.insert(name.clone(), p.clone());
            let nl: ShortCStr = c"\n".into();
            let line = ShortCStr::concat(&[&p, &nl]);
            let bytes = line.as_bytes().change_context(CmdError::Never)?;
            sys::OUT.write_all(bytes).change_context(CmdError::Never)?;
            Ok(0)
        }
        None => {
            report_not_found(name);
            Ok(1)
        }
    }
}

/// bash's `hash: NAME: not found` report — the one owner of the message, read
/// by `hash NAME` and `hash -d NAME` alike.
pub(super) fn report_not_found(name: &ShortCStr) {
    let _ = sys::ERR.write_all(b"hash: ");
    let _ = sys::ERR.write_all(name.as_bytes().unwrap_or(&[]));
    let _ = sys::ERR.write_all(b": not found\n");
}
