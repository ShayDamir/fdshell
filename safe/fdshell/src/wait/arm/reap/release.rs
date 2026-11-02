//! The release half of the arm's keep/release protocol: what an arm gives up
//! when it does not keep the fd it matched.

use crate::state::ShellState;
use crate::wait::round::ReleaseKey;

impl ShellState {
    /// Close the descriptor an arm chose to release, per the keep/release protocol.
    pub(super) fn apply_release(&mut self, key: &ReleaseKey) {
        match key {
            ReleaseKey::None => {}
            ReleaseKey::Var(name) => {
                self.fds.remove(name);
                self.arrays.remove(name);
            }
            ReleaseKey::Array { arr, source } => {
                self.remove_array_entry(arr, source);
            }
            ReleaseKey::Task(_) => {}
        }
    }
}
