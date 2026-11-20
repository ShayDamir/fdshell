#![allow(clippy::unwrap_used)]

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::state::ShellState;
use sys::ImportedStr;
use sys::fork_cell::ForkCell;

/// A state with the cached `interactive` flag the tty gate reads — the data
/// `seed_state` seeds from the `is_tty` syscall, so the gate is pinnable
/// in-process (LESSONS: caching a syscall's answer moves the branch into data).
fn cell(interactive: bool) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().interactive = interactive;
    cell
}

/// A variable in the store the shared lookup reads (an assignment lands here).
fn set(cell: &ForkCell<ShellState>, name: &'static CStr, value: &'static CStr) {
    cell.borrow_mut()
        .unwrap()
        .set_var(name.into(), ImportedStr::shell(value.into()));
}

/// The drawer's entry, the bytes it writes to stderr (the write itself is the
/// pty rows' in `tests/repl.rs`; the pty harness merges the two streams, so it
/// separates them only where a row diverts stdout to a file).
fn ps0(cell: &ForkCell<ShellState>) -> Vec<u8> {
    super::resolve(cell).unwrap()
}

#[test]
fn a_set_value_is_used_verbatim() {
    let cell = cell(true);
    set(&cell, c"PS0", c"[p0] ");
    assert_eq!(ps0(&cell), b"[p0] ");
}

#[test]
fn unset_and_set_empty_answer_nothing() {
    // The one prompt variable with **no** default string: bash seeds `PS1`/`PS2`
    // and `PS4` as real variables and falls back to a default when one is unset,
    // while `PS0` answers nothing unset and nothing empty (measured, bash 5.3.9).
    assert_eq!(ps0(&cell(true)), b"");
    let cell = cell(true);
    set(&cell, c"PS0", c"");
    assert_eq!(ps0(&cell), b"");
}

#[test]
fn the_shopt_gates_the_expansion() {
    // `promptvars` on — the default — expands; off pastes the stored bytes as
    // they stand while the decoder's work stays (`\a` lands as BEL either way),
    // the same split `repl/prompt/tests.rs` pins for `PS1` and `PS4`.
    let cell = cell(true);
    set(&cell, c"v", c"V");
    set(&cell, c"PS0", c"a\\a[$v]");
    assert_eq!(ps0(&cell), b"a\x07[V]");
    cell.borrow_mut().unwrap().options &= !crate::options::PROMPTVARS;
    assert_eq!(ps0(&cell), b"a\x07[$v]");
}

#[test]
fn a_stray_marker_keeps_its_readline_byte() {
    // `Markers::Bytes`, the `PS1`/`PS2` policy: a marker that does **not** toggle
    // the region is data and its byte is readline's — `a\]b` answers `a\x02b` here,
    // byte-identical to bash's measured display. `PS4` drops the same escape
    // (`repl/prompt/tests.rs`'s mirror row) because stderr is a raw drawer there,
    // while the policy stays the drawer's own. An octal `\001` is data, not a
    // marker, and the two policies — and bash — keep it.
    let cell = cell(true);
    set(&cell, c"PS0", c"a\\]b");
    assert_eq!(ps0(&cell), b"a\x02b");
    set(&cell, c"PS0", c"a\\001b");
    assert_eq!(ps0(&cell), b"a\x01b");
}

#[test]
fn a_toggling_marker_contributes_no_bytes() {
    // The arm the policy above cannot cover: a marker that **does** toggle is a
    // record split and the decoder consumes it (`decode.rs`'s state machine), so a
    // region's bytes survive and its markers' bytes do not — `A\[B\]C` answers `ABC`
    // and `x\[y` answers `xy`. bash's measured `PS0` displays are `A\001B\002C` and
    // `x\001y`: its display is a raw `fprintf` of the decoded string, which still
    // carries readline's two bytes, whereas the two prompts' displays lose them to
    // readline — the reason the record keeps the split (epic #198's width math) and
    // why the byte is the record's boundary here. `PS1`'s pty row
    // `a_non_printing_region_is_drawn_and_recorded` is the same fact one drawer up;
    // the divergence is #623's wording, pinned end to end by the pty row of this
    // name.
    let cell = cell(true);
    set(&cell, c"PS0", c"A\\[B\\]C");
    assert_eq!(ps0(&cell), b"ABC");
    set(&cell, c"PS0", c"x\\[y");
    assert_eq!(ps0(&cell), b"xy");
}

#[test]
fn off_a_terminal_answers_nothing() {
    // The tty gate is `PS1`/`PS2`'s and `PS0` shares it (bash on a pipe shows
    // nothing; `bash -i` forced over a pipe does show it, so the gate is
    // interactivity, not the stream).
    assert_eq!(ps0(&cell(false)), b"");
    let cell = cell(false);
    set(&cell, c"PS0", c"[p0] ");
    assert_eq!(ps0(&cell), b"");
}
