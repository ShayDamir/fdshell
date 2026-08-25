//! xtrace (`set -x` / `set +x`): print `+ <name> <args>` to stderr at each
//! dispatch entry, before the command runs.
//!
//! The PS4 prefix is the default `+ `. Child dispatch prints substituted
//! (expanded) arguments; intercepts print the raw arguments.
//!
//! Security note: expanded values can carry secrets into stderr.

use alloc::vec::Vec;

use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// xtrace a command from a `ForkCell` (shared borrow; silently skips on
/// borrow conflict).
pub(crate) fn trace_cmd(
    name: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) {
    let Ok(state) = cell.borrow() else {
        return;
    };
    trace(name, &cmdline.args, &state);
}

pub(crate) fn trace(name: &[u8], args: &[ShortCStr], state: &ShellState) {
    if state.options & crate::options::XTRACE == 0 {
        return;
    }
    let _ = sys::ERR.write_all(&format_trace(name, args));
}

/// Trace a command regardless of the xtrace option — for `set -x`/`set +x`
/// themselves, which print in both directions, as in bash.
pub(crate) fn trace_unconditional(name: &[u8], args: &[ShortCStr]) {
    let _ = sys::ERR.write_all(&format_trace(name, args));
}

fn format_trace(name: &[u8], args: &[ShortCStr]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"+ ");
    out.extend_from_slice(name);
    for arg in args {
        out.push(b' ');
        out.extend_from_slice(arg.as_bytes().unwrap_or(&[]));
    }
    out.push(b'\n');
    out
}
