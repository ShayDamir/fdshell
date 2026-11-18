// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Task #619's provider: the values of `\w` and `\W`, answered from the
//! `%CWD` fd's stored provenance — the `Origin` `cd.rs::move_cwd` records —
//! never by re-resolving a filesystem path (epic #197's rule, and LESSONS:
//! "The cwd's printable path is `getcwd`, not its `Origin`" is about the
//! *printed* path; the prompt is a decorator input and reads the provenance).
//!
//! Every source is a `ShellState` read (`fd_var`, `var_value`) and this
//! provider makes no syscall, so the seam's "syscall arms never borrow, borrow
//! arms never syscall" invariant (LESSONS: "The value provider answers per
//! escape, and a pty pin computes its expectation") holds trivially — the
//! three state reads for one escape (origin, `HOME`, `PROMPT_DIRTRIM`) are the
//! whole data flow. An origin that names no path answers `None` — decision
//! D2's "no value", drawn as the escape's own text (bash draws its logical
//! `$PWD` there; the divergence #623 documents). The display rules live in
//! `display.rs`.

pub(crate) mod display;

use alloc::vec::Vec;

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::Values;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// The two working-directory escapes, answered through the seam one escape at
/// a time (decision D3's lazy shape) from the `%CWD` fd's trace.
pub(crate) struct CwdValues<'a> {
    cell: &'a ForkCell<ShellState>,
}

impl<'a> CwdValues<'a> {
    pub(crate) fn new(cell: &'a ForkCell<ShellState>) -> Self {
        Self { cell }
    }
}

impl Values for CwdValues<'_> {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>> {
        if !matches!(esc, ValueEscape::WorkingDir | ValueEscape::ShortWorkingDir) {
            return None;
        }
        let state = self.cell.borrow().ok()?;
        display::answer(&state, esc)
    }
}

#[cfg(test)]
mod tests;
