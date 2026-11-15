//! The trap table: condition number → the disposition the shell holds for it.
//! Keys are signal numbers `1..=64` plus the [`EXIT`] slot `0`: POSIX's `EXIT`
//! is not a signal, so its entry is storage for the executor and is never
//! `sigaction`ed (installing dispositions is the `trap` builtin's job, task
//! #584, through [`sys::signal::set_action`], which answers `EINVAL` for `0`).
//! This module is storage only — insert/replace, reset, iterate; running a
//! stored body is the deferred executor's (task #585).

// mutants: -C --lib -- -E 'test(/^state::traps::/)'

use alloc::vec::Vec;

use sys::ShortCStr;

use super::ShellState;

/// POSIX's `EXIT` condition: the table's slot `0`, the only key that is not a
/// signal. It is never `sigaction`ed — `sigaction` accepts `1..=64` and answers
/// `EINVAL` for `0` — so this entry is only ever read by the executor.
pub(crate) const EXIT: i32 = 0;

/// What one condition does. Every state has **one** representation, so a lookup
/// has one answer: *default* is an **absent entry** (bash's `trap` list prints
/// nothing for it), *ignore* is the [`TrapDisposition::Ignore`] variant —
/// POSIX's ignore form is an empty action, and [`ShellState::set_trap`]
/// normalizes it on the way in — and *action* is
/// [`TrapDisposition::Action`] with non-empty bytes.
///
/// The bytes are kept **verbatim**: `Display` on `ShortCStr` is lossy, so the
/// list form prints the bytes, and `save_traps=$(trap); eval "$save_traps"`
/// round-trips.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) enum TrapDisposition {
    /// The kernel's own action for the condition: never stored, an absent entry
    /// answers it (see [`ShellState::trap_at`]).
    Default,
    /// Discard the condition — POSIX's ignore form, `trap '' COND`.
    Ignore,
    /// Run this body when the condition fires.
    Action(ShortCStr),
}

/// The store's own shape of `disposition`: `Default` has no entry, POSIX's
/// empty action is `Ignore`. `Some` means "store this", `None` means "the
/// condition has no entry"; [`ShellState::set_trap`] is its only caller, so the
/// two normalizations have one owner.
fn normalize(disposition: TrapDisposition) -> Option<TrapDisposition> {
    match disposition {
        TrapDisposition::Default => None,
        TrapDisposition::Ignore => Some(TrapDisposition::Ignore),
        TrapDisposition::Action(body) if body.is_empty() => Some(TrapDisposition::Ignore),
        TrapDisposition::Action(body) => Some(TrapDisposition::Action(body)),
    }
}

impl ShellState {
    /// Register `cond`'s disposition, **replacing** an earlier registration
    /// (bash: re-`trap`ping a condition replaces its action). `cond` is any
    /// condition number the caller has parsed, including [`EXIT`]; validating
    /// it is the builtin's job (task #584).
    pub(crate) fn set_trap(&mut self, cond: i32, disposition: TrapDisposition) {
        match normalize(disposition) {
            Some(disposition) => {
                self.traps.insert(cond, disposition);
            }
            None => {
                let _ = self.traps.remove(&cond);
            }
        }
    }

    /// POSIX's `trap - COND`: drop `cond`'s entry so it answers
    /// [`TrapDisposition::Default`] again. Delegates to
    /// [`ShellState::set_trap`], the single owner of the removal rule.
    pub(crate) fn reset_trap(&mut self, cond: i32) {
        self.set_trap(cond, TrapDisposition::Default);
    }

    /// `cond`'s disposition. An unregistered condition answers
    /// [`TrapDisposition::Default`] — the read half of the one-representation
    /// rule documented on [`TrapDisposition`].
    pub(crate) fn trap_at(&self, cond: i32) -> TrapDisposition {
        self.traps
            .get(&cond)
            .cloned()
            .unwrap_or(TrapDisposition::Default)
    }

    /// Every **registered** condition, in the order the list form prints it:
    /// ascending condition number, which puts [`EXIT`] first (bash 5.3.9,
    /// measured) — the order is a property of the store, so the `BTreeMap`'s
    /// ascending order *is* it.
    pub(crate) fn trap_list(&self) -> Vec<(i32, &TrapDisposition)> {
        self.traps.iter().map(|(cond, d)| (*cond, d)).collect()
    }
}

#[cfg(test)]
mod tests;
