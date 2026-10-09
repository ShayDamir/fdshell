//! `hash`'s pin/lookup path: `hash name` prints the entry (PATH-searching and
//! storing on a miss); `hash name path` pins the entry.

use crate::bytes::fold::fold_word;
use crate::error::cmd::CmdError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// `hash name` prints the entry (PATH-searching and storing on a miss);
/// `hash name path` pins the entry. `name` is the folded word (the caller folds
/// it, POSIX #4.1).
pub(super) fn lookup_or_pin(
    name: &ShortCStr,
    cmdline: &crate::parse::CommandLine,
    state: &mut ShellState,
) -> Result<i32, Report<CmdError>> {
    match cmdline.args.get(1) {
        Some(path) => {
            if cmdline.args.get(2).is_some() {
                bail!(CmdError::HashUsage);
            }
            // The pinned path is raw word text as well, so store the folded form.
            let path_mask = cmdline.args_mask.get(1).map(Vec::as_slice).unwrap_or(&[]);
            let path = fold_word(path, path_mask)
                .change_context(CmdError::Never)?
                .0;
            state.hash_table.insert(name.clone(), path);
            Ok(0)
        }
        None => {
            let path = state
                .hash_table
                .get(name)
                .cloned()
                .or_else(|| crate::exec::resolve_path_str(name, &state.hash_table).ok());
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
                    let _ = sys::ERR.write_all(b"hash: ");
                    let _ = sys::ERR.write_all(name.as_bytes().unwrap_or(&[]));
                    let _ = sys::ERR.write_all(b": not found\n");
                    Ok(1)
                }
            }
        }
    }
}
