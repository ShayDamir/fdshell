//! `set` listing and option forms: bare `set` (variables), `set -F`
//! (fd variables), and `set -o`/`+o` (options).

use alloc::vec::Vec;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

fn sort_by_bytes(names: &mut Vec<&ShortCStr>) {
    names.sort_by(|a, b| a.as_bytes().unwrap_or(&[]).cmp(b.as_bytes().unwrap_or(&[])));
}

/// Positional parameters (raw values) followed by `NAME=value` lines for all
/// string variables and exports, sorted by name.
pub(super) fn list_vars(cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let mut out = Vec::new();
    for p in &state.positional {
        out.extend_from_slice(p.value.as_bytes().change_context(CmdError::Resolve)?);
        out.push(b'\n');
    }
    let mut names: Vec<&ShortCStr> = state.strings.keys().collect();
    let ifs: ShortCStr = c"IFS".into();
    // `IFS` lives in `state.ifs` until first assigned; always list it (bash).
    if !names.iter().any(|n| n.eq_bytes(b"IFS")) {
        names.push(&ifs);
    }
    for name in state.exports.keys() {
        let bytes = name.as_bytes().change_context(CmdError::Resolve)?;
        if !names.iter().any(|n| n.eq_bytes(bytes)) {
            names.push(name);
        }
    }
    sort_by_bytes(&mut names);
    for name in names {
        let value = if name.eq_bytes(b"IFS") {
            state.ifs.clone()
        } else {
            state
                .strings
                .get(name)
                .or_else(|| state.exports.get(name))
                .ok_or(CmdError::Never)?
                .value
                .clone()
        };
        out.extend_from_slice(name.as_bytes().change_context(CmdError::Resolve)?);
        out.push(b'=');
        out.extend_from_slice(value.as_bytes().change_context(CmdError::Resolve)?);
        out.push(b'\n');
    }
    sys::OUT.write_all(&out).ok();
    state.set_last_exit(0);
    Ok(())
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
