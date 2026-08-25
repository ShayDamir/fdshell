use alloc::vec::Vec;

use sys::LocalFd;
use sys::ShortCStr;
use sys::Trace;

use super::{FdArrayEntry, FdVar, ShellState};

impl ShellState {
    /// Replace any value (scalar or array) of `name` with an empty array.
    pub fn set_empty_array(&mut self, name: ShortCStr) {
        self.fds.remove(&name);
        self.arrays.insert(name, Vec::new());
    }

    /// Append an owned descriptor to the array `name`, creating it if needed.
    pub fn append_array_entry(
        &mut self,
        name: &ShortCStr,
        fd: LocalFd,
        source: &ShortCStr,
        trace: Trace,
    ) {
        let entry = FdArrayEntry {
            fd,
            source: source.clone(),
            trace,
        };
        match self.arrays.get_mut(name) {
            Some(arr) => arr.push(entry),
            None => {
                self.arrays.insert(name.clone(), alloc::vec![entry]);
            }
        }
    }

    /// Remove the first entry of array `name` whose provenance matches `source`.
    pub fn remove_array_entry(&mut self, name: &ShortCStr, source: &ShortCStr) {
        let Some(arr) = self.arrays.get_mut(name) else {
            return;
        };
        let Some(i) = arr.iter().position(|e| e.source == *source) else {
            return;
        };
        arr.remove(i);
    }

    /// Store a scalar fd variable, closing any array of the same name first.
    pub fn set_fd_var(&mut self, name: ShortCStr, var: FdVar) {
        self.arrays.remove(&name);
        self.fds.insert(name, var);
    }

    /// Append captured entries to the array `name`, replacing any scalar.
    pub fn commit_captured_array(&mut self, name: ShortCStr, entries: Vec<FdArrayEntry>) {
        self.fds.remove(&name);
        match self.arrays.get_mut(&name) {
            Some(arr) => arr.extend(entries),
            None => {
                self.arrays.insert(name, entries);
            }
        }
    }
}
