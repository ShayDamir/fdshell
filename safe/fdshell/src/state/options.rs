// mutants: -C --lib -- -E 'test(/^state::options::/)'

use crate::options;

use super::ShellState;

impl ShellState {
    /// Whether this option bit is on — the state answers for its own `options`
    /// field (the fourteen copy-pasted `options & FLAG != 0` reads call this).
    pub(crate) fn option_on(&self, bit: u32) -> bool {
        self.options & bit != 0
    }

    /// The `strict` capability mode (README "Strict mode"): bans for the
    /// `*at` builtins, redirect targets, and external program paths.
    pub(crate) fn strict(&self) -> bool {
        self.option_on(options::STRICT)
    }
}

#[cfg(test)]
mod tests;
