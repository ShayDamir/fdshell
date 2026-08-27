//! `hash [-r] [name [path]]` — the command lookup table (bash compat).
//!
//! External lookups consult the table before PATH (`exec::resolve_path_str`)
//! and successful PATH searches store their result, so a pinned or cached
//! path skips the scan. Args are not expanded (like `set -o` names).

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, bail};
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
        None => list(&state),
        Some(flag) if flag.eq_bytes(b"-r") => remove(&mut state, cmdline),
        Some(name) => lookup_or_pin(name, cmdline, &mut state)?,
    };
    state.set_last_exit(exit);
    Ok(true)
}

/// Bare `hash`: one `name<TAB>path` line per entry.
fn list(state: &ShellState) -> i32 {
    for (name, path) in &state.hash_table {
        let _ = write_line(name, path);
    }
    0
}

fn write_line(name: &ShortCStr, path: &ShortCStr) -> Result<(), Report<CmdError>> {
    let tab: ShortCStr = c"\t".into();
    let nl: ShortCStr = c"\n".into();
    let line = ShortCStr::concat(&[name, &tab, path, &nl]);
    let bytes = line.as_bytes().change_context(CmdError::Never)?;
    sys::OUT.write_all(bytes).change_context(CmdError::Never)?;
    Ok(())
}

/// `hash -r [name…]`: clear the given entries, or the whole table.
fn remove(state: &mut ShellState, cmdline: &crate::parse::CommandLine) -> i32 {
    match cmdline.args.get(1..).unwrap_or(&[]) {
        [] => state.hash_table.clear(),
        names => {
            for name in names {
                state.hash_table.remove(name);
            }
        }
    }
    0
}

/// `hash name` prints the entry (PATH-searching and storing on a miss);
/// `hash name path` pins the entry.
fn lookup_or_pin(
    name: &ShortCStr,
    cmdline: &crate::parse::CommandLine,
    state: &mut ShellState,
) -> Result<i32, Report<CmdError>> {
    match cmdline.args.get(1) {
        Some(path) => {
            if cmdline.args.get(2).is_some() {
                bail!(CmdError::HashUsage);
            }
            state.hash_table.insert(name.clone(), path.clone());
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
