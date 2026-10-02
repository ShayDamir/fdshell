//! `hash [-r] [name [path]]` — the command lookup table (bash compat).
//!
//! External lookups consult the table before PATH (`exec::resolve_path_str`)
//! and successful PATH searches store their result, so a pinned or cached
//! path skips the scan. Args are not expanded (like `set -o` names).

mod pin;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
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
    let exit = match cmdline.args.first() {
        None => state.list_hash(),
        Some(flag) if flag.eq_bytes(b"-r") => {
            state.remove_hash(cmdline.args.get(1..).unwrap_or(&[]))
        }
        Some(name) => pin::lookup_or_pin(name, cmdline, &mut state)?,
    };
    state.set_last_exit(exit);
    Ok(true)
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
