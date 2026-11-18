// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! The provider seam for value escapes (decision D3): an object-safe trait,
//! so neither `run.rs` nor the REPL ever names a concrete provider.

use alloc::vec::Vec;

use crate::repl::prompt::segment::ValueEscape;

pub(crate) mod clock;
pub(crate) mod composite;
pub(crate) mod cwd;
pub(crate) mod shell;

/// What a provider answers for one value escape; `None` means "no value" and
/// the renderer falls back to the placeholder's literal escape text
/// (decision D2). The REPL renders through [`composite`], which asks [`shell`],
/// [`clock`] and [`cwd`] in turn — one provider per escape family (task #619's
/// `cwd.rs` completed the set).
pub(crate) trait Values {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>>;
}

/// The provider answering `None` for everything, so every placeholder renders
/// literally. It stays as the seam's null provider — the render tests' stand-in,
/// and the shape a provider that owns none of the escapes takes.
pub(crate) struct NoValues;

impl Values for NoValues {
    fn value(&self, _esc: ValueEscape) -> Option<Vec<u8>> {
        None
    }
}
