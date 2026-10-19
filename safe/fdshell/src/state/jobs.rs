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
/// The tables stay private — callers go through the getters and the removals
/// below, so no caller can reindex or reorder a job behind `wait`'s back.
pub(crate) struct Jobs {
    named: HashMap<ShortCStr, Task>,
    anon: VecDeque<Task>,
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

    /// The named job stored under `name` (`wait { &task }` arms poll its pidfd).
    pub(crate) fn get_named(&self, name: &ShortCStr) -> Option<&Task> {
        self.named.get(name)
    }

    /// `true` when the named table holds `name`.
    pub(crate) fn contains_named(&self, name: &ShortCStr) -> bool {
        self.named.contains_key(name)
    }

    /// The named table's job count.
    pub(crate) fn named_len(&self) -> usize {
        self.named.len()
    }

    /// The nameless deque's job count.
    pub(crate) fn anon_len(&self) -> usize {
        self.anon.len()
    }

    /// The nameless jobs' child pids, in FIFO order (`$!` is the last one).
    pub(crate) fn anon_pids(&self) -> Vec<sys::Pid> {
        self.anon.iter().map(|task| task.child_pid).collect()
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
        self.rotate_anon(at);
        self.anon.pop_front()
    }

    /// Drain the nameless deque in FIFO order (`wait` without operands).
    pub(crate) fn drain_anon(&mut self) -> VecDeque<Task> {
        core::mem::take(&mut self.anon)
    }

    /// Drain the named table (every job, in iteration order).
    pub(crate) fn drain_named(&mut self) -> Vec<Task> {
        core::mem::take(&mut self.named).into_values().collect()
    }

    /// `true` when neither table holds a job.
    pub(crate) fn is_empty(&self) -> bool {
        self.named.is_empty() && self.anon.is_empty()
    }

    /// Rotate the deque so the entry at `at` becomes its front, keeping the
    /// FIFO order of the rest (the entry `wait <pid>` is about to pop).
    fn rotate_anon(&mut self, at: usize) {
        let mut head: VecDeque<Task> = self.anon.drain(..at).collect();
        self.anon.append(&mut head);
    }
}

#[cfg(test)]
mod tests;
