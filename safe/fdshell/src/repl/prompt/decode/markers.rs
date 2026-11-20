// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'
//! The decoder's marker policy as data (task #621): whether a non-toggling
//! `\[…\]` escape contributes its readline byte to the record. Only the decoder
//! knows which bytes it inserted — an octal `\001` is data and stays under both
//! policies — so the question is answered at its entry and threaded down, the
//! `PromptVars` / task #371 pattern for a policy decided once at a surface.
//!
//! The two answers are the two drawers. `PS1`/`PS2` are drawn through readline,
//! which is where bash's measured `\001`/`\002` bytes come from; `PS4` is written
//! raw to stderr, where bash emits **nothing** for the same escape (measured
//! `a\]b` → `ab`), while a matched `\[`…`\]` pair still keeps its region bytes.

use super::Action;

/// What a non-toggling marker escape contributes to the record.
#[derive(Clone, Copy)]
pub(crate) enum Markers {
    /// The prompt drawer: the escape emits readline's
    /// `RL_PROMPT_{START,END}_IGNORE` byte (`PS1`/`PS2`).
    Bytes,

    /// The raw drawer: the escape is consumed and contributes no bytes (`PS4`).
    Dropped,
}

impl Markers {
    /// The action for a non-toggling marker escape whose readline byte is
    /// `byte`; a toggling marker is a split and never asks this question.
    pub(super) fn action(self, byte: u8) -> Action {
        match self {
            Self::Bytes => Action::Byte(byte),
            Self::Dropped => Action::Skipped,
        }
    }
}

#[cfg(test)]
mod tests;
