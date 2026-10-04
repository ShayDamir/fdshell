//! verbose (`set -v` / `set +v`): echo each statement to stderr before it
//! runs, as bash prints its input lines.
//!
//! fdshell echoes at execution time (bash at read time), so a single-line
//! `sh -c 'set -v; echo hi'` prints `echo hi` here and nothing in bash;
//! multi-line scripts and the REPL match bash.

use alloc::vec::Vec;

use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// Echo `bytes` to stderr when the verbose option is on (shared borrow;
/// silently skips on borrow conflict).
pub(crate) fn trace(bytes: &[u8], cell: &ForkCell<ShellState>) {
    let Ok(state) = cell.borrow() else {
        return;
    };
    if state.options & crate::options::VERBOSITY == 0 {
        return;
    }
    let mut out = Vec::from(bytes);
    out.push(b'\n');
    let _ = sys::ERR.write_all(&out);
}
