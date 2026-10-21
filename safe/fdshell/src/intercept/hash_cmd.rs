//! `hash [-rd] [name [path]]` — the command lookup table (bash compat).
//!
//! External lookups consult the table before PATH (`exec::resolve_path_str`)
//! and successful PATH searches store their result, so a pinned or cached
//! path skips the scan. Every word `hash` consumes is a **substitution**
//! surface: the words go through the argument pipeline (`substitute_args` —
//! tilde, `$name`, `$( … )`, backticks, `$(( … ))`, `%fd`, escape pairs, IFS
//! splitting, pathname expansion), and the options are parsed on the
//! **expanded** fields, as bash parses them after expansion. The table is
//! keyed by the expanded name, the same shape `prehash` stores for an
//! expanded word 0, so `hash a\*` and `a\*` meet on the key `a*`.

mod drop;
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
    // The expansion re-borrows the `ForkCell` (IFS, `dotglob`, `%fd`), so it
    // runs **before** the state borrow — never across it (LESSONS "Never hold
    // a ForkCell borrow across a recursive call that re-borrows").
    let words = crate::substitute::substitute_args(
        &cmdline.args,
        &cmdline.args_mask,
        &cmdline.args_span,
        &cmdline.args_quoted,
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let exit = dispatch(&words, &mut state)?;
    state.set_last_exit(exit);
    Ok(true)
}

/// Options are read on the expanded fields: `hash $(echo -r)` clears the
/// table, `hash -d $(echo ls)` deletes the expanded name. `names` is the
/// operand list after the option word (or after the name, for the pin form).
fn dispatch(names_all: &[ShortCStr], state: &mut ShellState) -> Result<i32, Report<CmdError>> {
    let operands = names_all.get(1..).unwrap_or(&[]);
    match names_all.first() {
        None => Ok(state.list_hash()),
        Some(flag) if flag.eq_bytes(b"-r") => Ok(state.remove_hash(operands)),
        Some(flag) if flag.eq_bytes(b"-d") => drop::delete_names(operands, state),
        Some(name) => pin::lookup_or_pin(name, operands, state),
    }
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

#[cfg(test)]
mod tests;
