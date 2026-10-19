//! The background-job tables (task #326, STYLE §2.6 extraction from
//! `state.rs`): the **named** jobs launched by `&>&name`/`&>|&name` pidvar
//! redirects, keyed by task name, and the **nameless** jobs launched by the
//! plain `&` async operator, a FIFO deque `wait` reaps. `unset` detaches a
//! nameless job from nothing — it only ever removes the named table's entry
//! for the unset name.

use crate::task::Task;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use hashbrown::HashMap;
use sys::ShortCStr;

/// Both background-job tables: named (pidvar form) and anonymous (plain `&`).
pub(crate) struct Jobs {
    pub(crate) named: HashMap<ShortCStr, Task>,
    pub(crate) anon: VecDeque<Task>,
}

impl Jobs {
    /// Both tables empty.
    pub(crate) fn new() -> Self {
        Self {
            named: HashMap::new(),
            anon: VecDeque::new(),
        }
    }

    /// Insert (replacing) a named pidvar job under `name`.
    pub(crate) fn insert_named(&mut self, name: ShortCStr, task: Task) {
        self.named.insert(name, task);
    }

    /// Record a plain-`&` job at the tail of the nameless deque.
    pub(crate) fn push_anon(&mut self, task: Task) {
        self.anon.push_back(task);
    }

    /// Remove a named job by name (`unset`, `waitpid &name`).
    pub(crate) fn remove_named(&mut self, name: &ShortCStr) -> Option<Task> {
        self.named.remove(name)
    }

    /// The named table's keys, in iteration order (`wait` without operands).
    pub(crate) fn named_keys(&self) -> Vec<ShortCStr> {
        self.named.keys().cloned().collect()
    }

    /// Remove a job by child pid: the named table first (its keys are the
    /// `waitpid` handles), then the nameless deque (`wait $!` of a plain `&`).
    pub(crate) fn remove_by_pid(&mut self, pid: sys::Pid) -> Option<Task> {
        let key = self
            .named
            .iter()
            .find(|(_, task)| task.child_pid == pid)
            .map(|(key, _)| key.clone());
        if let Some(key) = key {
            return self.named.remove(&key);
        }
        let at = self.anon.iter().position(|task| task.child_pid == pid)?;
        let mut head: VecDeque<Task> = self.anon.drain(..at).collect();
        self.anon.append(&mut head);
        self.anon.pop_front()
    }

    /// Drain the nameless deque in FIFO order (`wait` without operands).
    pub(crate) fn drain_anon(&mut self) -> VecDeque<Task> {
        core::mem::take(&mut self.anon)
    }

    /// `true` when neither table holds a job.
    pub(crate) fn is_empty(&self) -> bool {
        self.named.is_empty() && self.anon.is_empty()
    }
}

#[cfg(test)]
mod tests;
