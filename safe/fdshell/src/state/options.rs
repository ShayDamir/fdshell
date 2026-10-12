use crate::options;

use super::ShellState;

impl ShellState {
    /// The `strict` capability mode (README "Strict mode"): bans for the
    /// `*at` builtins, redirect targets, and external program paths.
    pub(crate) fn strict(&self) -> bool {
        self.options & options::STRICT != 0
    }
}
