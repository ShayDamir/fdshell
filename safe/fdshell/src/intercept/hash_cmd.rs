//! `hash [-r] [name [path]]` — the command lookup table (bash compat).
//!
//! External lookups consult the table before PATH (`exec::resolve_path_str`)
//! and successful PATH searches store their result, so a pinned or cached
//! path skips the scan. Args are not expanded (like `set -o` names).

mod pin;

use crate::bytes::fold::fold_word;
use crate::error::cmd::CmdError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(crate) fn run_hash(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "hash", cmdline)?;
    crate::xtrace::trace_cmd(b"hash", cmdline, cell);
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    // `hash`'s words never go through substitution, so they are folded here
    // (POSIX #4.1): the table is keyed by the *folded* command name (`prehash`
    // stores word 0 folded), so `hash a\*` queries the key `a*` as bash's does.
    let exit = match cmdline.args.first() {
        None => state.list_hash(),
        Some(raw) => {
            let name = fold_name(raw, mask_at(&cmdline.args_mask, 0))?;
            if name.eq_bytes(b"-r") {
                state.remove_hash(&fold_names(
                    cmdline.args.get(1..).unwrap_or(&[]),
                    cmdline.args_mask.get(1..).unwrap_or(&[]),
                )?)
            } else {
                pin::lookup_or_pin(&name, cmdline, &mut state)?
            }
        }
    };
    state.set_last_exit(exit);
    Ok(true)
}

fn mask_at(masks: &[Vec<bool>], i: usize) -> &[bool] {
    masks.get(i).map(Vec::as_slice).unwrap_or(&[])
}

/// The folded name word, as the table's key.
fn fold_name(word: &ShortCStr, mask: &[bool]) -> Result<ShortCStr, Report<CmdError>> {
    Ok(fold_word(word, mask).change_context(CmdError::Never)?.0)
}

/// Every name of `hash -r name…`, folded to match the table's folded keys.
fn fold_names(
    words: &[ShortCStr],
    masks: &[Vec<bool>],
) -> Result<Vec<ShortCStr>, Report<CmdError>> {
    words
        .iter()
        .zip(masks)
        .map(|(w, m)| fold_name(w, m))
        .collect()
}

impl ShellState {
    /// Bare `hash`: one `name<TAB>path` line per entry.
    fn list_hash(&self) -> i32 {
        for (name, path) in &self.hash_table {
            let _ = write_line(name, path);
        }
        0
    }

    /// `hash -r [name…]`: clear the given entries, or the whole table.
    fn remove_hash(&mut self, names: &[ShortCStr]) -> i32 {
        match names {
            [] => self.hash_table.clear(),
            names => {
                for name in names {
                    self.hash_table.remove(name);
                }
            }
        }
        0
    }
}

fn write_line(name: &ShortCStr, path: &ShortCStr) -> Result<(), Report<CmdError>> {
    let tab: ShortCStr = c"\t".into();
    let nl: ShortCStr = c"\n".into();
    let line = ShortCStr::concat(&[name, &tab, path, &nl]);
    let bytes = line.as_bytes().change_context(CmdError::Never)?;
    sys::OUT.write_all(bytes).change_context(CmdError::Never)?;
    Ok(())
}
