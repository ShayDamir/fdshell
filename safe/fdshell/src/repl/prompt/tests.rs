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
    // The bash default is all value escapes, and they now answer real values
    // (task #617): the program name the REPL seeds, the crate's own version, and
    // the uid's character. The runner is non-root — as bash's measured default
    // already is — and bash draws its own identity there (`bash-5.3$ `,
    // 2026-11-18), the divergence #623 documents.
    assert_eq!(
        resolve(&cell(true), c"PS1", DEFAULT),
        concat!("fdshell-", env!("CARGO_PKG_VERSION"), "$ ")
            .as_bytes()
            .to_vec()
    );
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

#[test]
fn the_resolver_decodes_a_mechanical_escape() {
    // The resolver returns the decoded-and-rendered bytes: `\a` lands as BEL.
    let cell = cell(true);
    set(&cell, c"PS1", c"a\\ab");
    assert_eq!(resolve(&cell, c"PS1", DEFAULT), b"a\x07b");
}

#[test]
fn a_value_escape_renders_as_its_literal_text() {
    // Decision D2 through the composite: an escape whose owner has no value yet
    // answers `None` and draws its own text — here #619's `\w`, the only escape
    // left without an owner, so this row moves again when #619 lands. The
    // permanent D2 pin is `render/tests.rs`'s `NoValues` row.
    let cell = cell(true);
    set(&cell, c"PS1", c"a\\wb");
    assert_eq!(resolve(&cell, c"PS1", DEFAULT), br"a\wb");
}

#[test]
fn the_resolver_answers_a_clock_escape() {
    // Through the composite the resolver reaches the clock provider, and the
    // machine's own clock rides into the value — so this row pins the shape
    // (LESSONS: "A test that measures wall-clock under load cannot pin the
    // number"); exact bytes are pinned at injected instants in
    // `value/clock/tests.rs`.
    let cell = cell(true);
    set(&cell, c"PS1", c"a\\tb");
    let out = resolve(&cell, c"PS1", DEFAULT);
    let mid_ok = out.get(1..9).is_some_and(|mid| {
        mid.iter().enumerate().all(|(i, &b)| {
            if matches!(i, 2 | 5) {
                b == b':'
            } else {
                b.is_ascii_digit()
            }
        })
    });
    assert!(
        out.len() == 10 && out.starts_with(b"a") && out.ends_with(b"b") && mid_ok,
        "aHH:MM:SSb: {out:?}"
    );
}
