pub mod text;

pub use text::ScriptText;

use core::num::NonZeroU8;
use core::ops::Deref;

use crate::shortcstr::{NoNul, ShortCStr};

/// A 1-based position within a line of source text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// The source boundary a string value originated from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Process argument vector, 0-based index of the full argv.
    CliArgument(usize),
    /// Environment variable of the given name.
    EnvVar(ShortCStr),
    /// Content of the file at the given path.
    File(ShortCStr),
    /// Standard input.
    Stdin,
    /// Output of a command substitution.
    CommandOutput,
    /// Data read from the file descriptor of the given name.
    Read(ShortCStr),
    /// Produced by the shell itself (e.g. the default `$0`).
    Shell,
}

/// Provenance metadata for a traced string value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trace {
    /// Position in the source line where the value was set, if any.
    pub set_at: Option<Position>,
    /// The source boundary the value originated from.
    pub origin: Origin,
}

impl Trace {
    /// Trace for a boundary value (argv, initial environ, default `$0`).
    pub fn boundary(origin: Origin) -> Self {
        Self {
            set_at: None,
            origin,
        }
    }

    /// Trace for a value set at a source position.
    pub fn at(pos: Position, origin: Origin) -> Self {
        Self {
            set_at: Some(pos),
            origin,
        }
    }
}

/// A string value with its provenance trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedStr {
    pub value: ShortCStr,
    pub trace: Trace,
}

impl ImportedStr {
    pub fn new(value: ShortCStr, trace: Trace) -> Self {
        Self { value, trace }
    }

    /// A value produced by the shell itself (e.g. the default `$0`).
    pub fn shell(value: ShortCStr) -> Self {
        Self::new(value, Trace::boundary(Origin::Shell))
    }
}

impl Deref for ImportedStr {
    type Target = ShortCStr;
    fn deref(&self) -> &ShortCStr {
        &self.value
    }
}

// The value invariant guarantees no NUL bytes; delegation keeps the proof in one place.
impl NoNul for ImportedStr {
    fn as_non_zero_bytes(&self) -> &[NonZeroU8] {
        self.value.as_non_zero_bytes()
    }
}
