//! `set` listing and option forms: bare `set` (variables), `set -F`
//! (fd variables), and `set -o`/`+o` (options).

mod vars;

pub(super) use vars::list_vars;

use alloc::vec::Vec;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(super) fn sort_by_bytes(names: &mut Vec<&ShortCStr>) {
    names.sort_by(|a, b| a.as_bytes().unwrap_or(&[]).cmp(b.as_bytes().unwrap_or(&[])));
}

/// All fd variables as `%name` lines, sorted by name.
pub(super) fn list_fds(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let mut names: Vec<&ShortCStr> = state.fds.keys().chain(state.arrays.keys()).collect();
    sort_by_bytes(&mut names);
    names.dedup_by(|a, b| a.as_bytes().unwrap_or(&[]).eq(b.as_bytes().unwrap_or(&[])));
    let mut out = Vec::new();
    for name in names {
        out.push(b'%');
        out.extend_from_slice(name.as_bytes().change_context(CmdError::Resolve)?);
        out.push(b'\n');
    }
    sys::OUT.write_all(&out).ok();
    state.set_last_exit(0);
    Ok(())
}

/// `set -o name` enables, `set +o name` disables; bare `set -o` lists options.
pub(super) fn run_set_option(
    flag: &ShortCStr,
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let enable = flag.eq_bytes(b"-o");
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    match cmdline.args.get(1) {
        None => {
            sys::OUT
                .write_all(&crate::options::list(state.options))
                .ok();
            state.set_last_exit(0);
        }
        Some(name) => {
            let bit = crate::options::lookup(name).ok_or(CmdError::ShellOptionUnknown {
                command: "set",
                name: name.clone(),
            })?;
            state.options = crate::options::set(state.options, bit, enable);
            state.set_last_exit(0);
        }
    }
    Ok(())
}
