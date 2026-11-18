#![allow(clippy::unwrap_used)]

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ShortCStr};

/// bash 5.3.9's default `PS1`, the `run.rs` constant the call sites pass.
const DEFAULT: &[u8] = b"\\s-\\v\\$ ";

/// A state with the cached `interactive` flag the tty gate reads — the data
/// `seed_state` seeds from the `is_tty` syscall, so the gate is pinnable
/// in-process (LESSONS: caching a syscall's answer moves the branch into data).
fn cell(interactive: bool) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().interactive = interactive;
    cell
}

/// A variable in the store the shared lookup reads (an assignment or an
/// `export` lands here; the inherited `environ` half is pinned end-to-end).
fn set(cell: &ForkCell<ShellState>, name: &'static CStr, value: &'static CStr) {
    cell.borrow_mut()
        .unwrap()
        .set_var(name.into(), ImportedStr::shell(value.into()));
}

fn resolve(cell: &ForkCell<ShellState>, name: &'static CStr, default: &[u8]) -> Vec<u8> {
    super::resolve(cell, &ShortCStr::from(name), default).unwrap()
}

#[test]
fn unset_falls_back_to_the_default() {
    assert_eq!(resolve(&cell(true), c"PS1", DEFAULT), DEFAULT);
    assert_eq!(resolve(&cell(true), c"PS2", b"> "), b"> ");
}

#[test]
fn a_set_value_is_drawn_verbatim() {
    let cell = cell(true);
    set(&cell, c"PS1", c"[p1] ");
    assert_eq!(resolve(&cell, c"PS1", DEFAULT), b"[p1] ");
}

#[test]
fn a_set_empty_value_draws_nothing() {
    // The distinction the default-as-read-time-fallback exists for: bash
    // measures `PS1=""` as no prompt at all, not the default.
    let cell = cell(true);
    set(&cell, c"PS1", c"");
    assert_eq!(resolve(&cell, c"PS1", DEFAULT), b"");
}

#[test]
fn off_a_terminal_nothing_is_drawn() {
    // bash draws no prompt off-terminal, whether the variable is set or not.
    assert_eq!(resolve(&cell(false), c"PS1", DEFAULT), b"");
    let cell = cell(false);
    set(&cell, c"PS1", c"[p1] ");
    assert_eq!(resolve(&cell, c"PS1", DEFAULT), b"");
}

#[test]
fn a_terminal_draws_the_value_not_the_default() {
    // The gate is the flag, not the value: with the variable set on a terminal
    // the value wins, and only an unset variable falls back.
    let cell = cell(true);
    set(&cell, c"PS2", c"| ");
    assert_eq!(resolve(&cell, c"PS2", b"> "), b"| ");
}
