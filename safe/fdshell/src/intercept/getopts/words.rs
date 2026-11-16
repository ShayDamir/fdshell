// mutants: -C --lib -- -E 'test(/^intercept::getopts::/)'

//! The word list and its index rules (task #590): `getopts` processes the given
//! `arg…` operands when present, otherwise the positional parameters. Indices are
//! **1-based** into the chosen list; index 0 is read as word 1, and the
//! end-of-options position is clamped to `$#`+1 (the shape bash 5.3.9 measures).

use alloc::collections::VecDeque;

use sys::{ImportedStr, ShortCStr};

/// The words one `getopts` call scans, borrowed from the driver.
pub(crate) enum Words<'a> {
    Operands(&'a [ShortCStr]),
    Positional(&'a VecDeque<ImportedStr>),
}

impl<'a> Words<'a> {
    /// The operand list wins when it is not empty; the positional parameters
    /// serve otherwise.
    pub(crate) fn resolve(
        operands: &'a [ShortCStr],
        positional: &'a VecDeque<ImportedStr>,
    ) -> Self {
        if operands.is_empty() {
            Self::Positional(positional)
        } else {
            Self::Operands(operands)
        }
    }

    /// Index 0 reads as word 1; every other index stands (bash measures
    /// `OPTIND=0` mid-scan as "re-read word 1 from its start").
    pub(crate) fn normalize(&self, index: u64) -> u64 {
        if index == 0 { 1 } else { index }
    }

    /// The word at the 1-based `index` (already normalized), `None` past the end.
    pub(crate) fn word(&self, index: u64) -> Option<ShortCStr> {
        let slot = usize::try_from(index).ok()?.checked_sub(1)?;
        match self {
            Self::Operands(words) => words.get(slot).cloned(),
            Self::Positional(words) => words.get(slot).map(|word| word.value.clone()),
        }
    }

    /// The clamped end-of-options index: `$#`+1.
    pub(crate) fn end_index(&self) -> u64 {
        u64::try_from(self.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1)
    }

    fn len(&self) -> usize {
        match self {
            Self::Operands(words) => words.len(),
            Self::Positional(words) => words.len(),
        }
    }
}

#[cfg(test)]
mod tests;
