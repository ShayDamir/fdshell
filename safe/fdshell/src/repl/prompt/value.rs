// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! The provider seam for value escapes (decision D3): an object-safe trait,
//! so neither `run.rs` nor the REPL ever names a concrete provider.

use alloc::vec::Vec;

use crate::repl::prompt::segment::ValueEscape;

/// What a provider answers for one value escape; `None` means "no value" and
/// the renderer falls back to the placeholder's literal escape text
/// (decision D2). #617/#618/#619 implement this over `ShellState`.
pub(crate) trait Values {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>>;
}

/// The provider the REPL uses until the value subtasks wire real ones: it
/// answers `None` for everything, so every placeholder renders literally.
pub(crate) struct NoValues;

impl Values for NoValues {
    fn value(&self, _esc: ValueEscape) -> Option<Vec<u8>> {
        None
    }
}
