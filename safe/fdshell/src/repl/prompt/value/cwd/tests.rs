#![allow(clippy::unwrap_used)]

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::Values;
use crate::repl::prompt::value::cwd::CwdValues;
use crate::state::{FdVar, ShellState};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, Position, Trace};

/// The `HOME` every pin seeds, so the `~` branch is driven by the store, not
/// by the machine.
const HOME: &CStr = c"/home/test";

/// A state whose `%CWD` fd carries `origin`, with `HOME` and — when `dirtrim`
/// names one — `PROMPT_DIRTRIM` in the variable store. The fd itself is a
/// memfd: the provider never touches it (no syscalls), the trace is the whole
/// input.
fn seeded(origin: Origin, dirtrim: Option<&'static CStr>) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    {
        let mut state = cell.borrow_mut().unwrap();
        let fd = sys::memfd::memfd_create().unwrap();
        state.set_fd_var(
            c"CWD".into(),
            FdVar {
                fd,
                trace: Trace::at(Position::new(1, 1), origin),
            },
        );
        state.set_var(c"HOME".into(), ImportedStr::shell(HOME.into()));
        if let Some(value) = dirtrim {
            state.set_var(c"PROMPT_DIRTRIM".into(), ImportedStr::shell(value.into()));
        }
    }
    cell
}

/// `(what \w draws, what \W draws)` through the provider, as the composite
/// asks it.
fn answers(cell: &ForkCell<ShellState>) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let provider = CwdValues::new(cell);
    (
        provider.value(ValueEscape::WorkingDir),
        provider.value(ValueEscape::ShortWorkingDir),
    )
}

#[test]
fn a_path_origin_draws_the_origin_itself() {
    // D-619-1: the recorded origin is the source, never a re-resolution (bash
    // draws its logical `$PWD` — the same bytes for an absolute origin).
    let cell = seeded(Origin::File(c"/tmp".into()), None);
    assert_eq!(
        answers(&cell),
        (Some(b"/tmp".to_vec()), Some(b"tmp".to_vec()))
    );
    // The measured `n=1 → .../last` row on a deeper origin.
    let cell = seeded(Origin::File(c"/a/b/c".into()), Some(c"1"));
    assert_eq!(
        answers(&cell),
        (Some(b".../c".to_vec()), Some(b"c".to_vec()))
    );
}

#[test]
fn a_relative_origin_draws_what_was_typed() {
    // D-619-3 pinned: `cd sub` records `File("sub")` and draws `sub`; bash
    // draws `/start/sub` — the divergence #623 documents. No logical-path
    // store is added for it (LESSONS: "The cwd's printable path is `getcwd`,
    // not its `Origin`" — and the prompt reads the `Origin`).
    let cell = seeded(Origin::File(c"sub".into()), None);
    assert_eq!(
        answers(&cell),
        (Some(b"sub".to_vec()), Some(b"sub".to_vec()))
    );
    let cell = seeded(Origin::File(c"a/b/c".into()), None);
    assert_eq!(
        answers(&cell),
        (Some(b"a/b/c".to_vec()), Some(b"c".to_vec()))
    );
}

#[test]
fn the_home_prefix_becomes_a_tilde() {
    // Measured `[~][~]` at `$HOME` and `[~/rust][rust]` under it (bash 5.3.9,
    // 2026-11-18); D-619-4 drives the abbreviation off the shell's own lookup,
    // so a store `HOME` moves it (LESSONS: "The tilde gate is mask-driven; the
    // `HOME` lookup is state-driven").
    let cell = seeded(Origin::File(HOME.into()), None);
    assert_eq!(answers(&cell), (Some(b"~".to_vec()), Some(b"~".to_vec())));
    let cell = seeded(Origin::File(c"/home/test/rust".into()), None);
    assert_eq!(
        answers(&cell),
        (Some(b"~/rust".to_vec()), Some(b"rust".to_vec()))
    );
    // A prefix-shaped name is not the home: `/home/testimony` must not abbreviate.
    let cell = seeded(Origin::File(c"/home/testimony/x".into()), None);
    assert_eq!(
        answers(&cell),
        (Some(b"/home/testimony/x".to_vec()), Some(b"x".to_vec()))
    );
}

#[test]
fn an_env_var_origin_answers_only_home() {
    // D-619-2: `EnvVar(HOME)` answers `~` — the two interactive routes `cd ~`
    // and `cd ~/x` land on bash parity; any other name names no path and
    // answers `None`.
    let cell = seeded(Origin::EnvVar(c"HOME".into()), None);
    assert_eq!(answers(&cell), (Some(b"~".to_vec()), Some(b"~".to_vec())));
    let cell = seeded(Origin::EnvVar(c"PATH".into()), None);
    assert_eq!(answers(&cell), (None, None));
}

#[test]
fn an_origin_without_a_path_answers_no_value() {
    // D-619-2 pinned: bash draws its `$PWD` here, fdshell answers D2's literal
    // (the pty row `an_origin_without_a_path_draws_the_escape_text` is the
    // end-to-end half; the startup `open(".")` fd's `Origin::Shell` is the row
    // that matters in production).
    for origin in [
        Origin::Shell,
        Origin::Captured(c"tag".into()),
        Origin::Read(c"f".into()),
        Origin::Stdin,
        Origin::CommandOutput,
        Origin::CliArgument(0),
    ] {
        let cell = seeded(origin, None);
        assert_eq!(answers(&cell), (None, None));
    }
    // No `%CWD` fd at all — the same answer before startup seeds one.
    let cell = ForkCell::new(ShellState::new());
    assert_eq!(answers(&cell), (None, None));
}

#[test]
fn dirtrim_keeps_the_last_n_components() {
    // The plain rule, on the measured 11-component rows (n=1 → `.../k` shape,
    // n=10 trims, count == n does not) and the measured one-component-drop
    // boundary, which **does** trim (`/tmp/opencode` at n=1 → `.../opencode`).
    let deep = Origin::File(c"/a/b/c/d/e/f/g/h/i/j/k".into());
    let full = b"/a/b/c/d/e/f/g/h/i/j/k".to_vec();
    let cell = seeded(deep.clone(), Some(c"1"));
    assert_eq!(answers(&cell).0, Some(b".../k".to_vec()));
    let cell = seeded(deep.clone(), Some(c"10"));
    assert_eq!(answers(&cell).0, Some(b".../b/c/d/e/f/g/h/i/j/k".to_vec()));
    // `PROMPT_DIRTRIM` cannot move `\W` — measured unchanged at every n.
    assert_eq!(answers(&cell).1, Some(b"k".to_vec()));
    let cell = seeded(deep, Some(c"11")); // count == n → no trim.
    assert_eq!(answers(&cell).0, Some(full));
    // The boundary that differs from the `~` branch (count == n + 1 trims):
    let cell = seeded(Origin::File(c"/opt/x".into()), Some(c"1"));
    assert_eq!(answers(&cell).0, Some(b".../x".to_vec()));
}

#[test]
fn dirtrim_after_a_tilde_needs_two_components_to_drop() {
    // The two-threshold pin (D-619-5): the measured rows trim at n = 1, 2, 3
    // on five components after `~`, not at n = 4 — a single-rule
    // "simplification" to `> n` would trim at 4 and fail this row.
    let rest5 = Origin::File(c"/home/test/tmp/pdt/1/2/3".into());
    for (n, want) in [
        (c"1", b"~/.../3".as_slice()),
        (c"2", b"~/.../2/3"),
        (c"3", b"~/.../1/2/3"),
        (c"4", b"~/tmp/pdt/1/2/3"),
    ] {
        assert_eq!(
            answers(&seeded(rest5.clone(), Some(n))).0,
            Some(want.to_vec()),
            "measured row n={n:?}"
        );
    }
    // The measured boundary rows: rest == n + 1 does **not** trim, rest == n + 2
    // does — the rows that prove the two thresholds are not one rule.
    let rest3 = Origin::File(c"/home/test/tmp/pdt/1".into());
    assert_eq!(
        answers(&seeded(rest3.clone(), Some(c"2"))).0,
        Some(b"~/tmp/pdt/1".to_vec())
    );
    assert_eq!(
        answers(&seeded(rest3, Some(c"1"))).0,
        Some(b"~/.../1".to_vec())
    );
    // `$HOME` itself cannot be trimmed, and neither can `/` (measured).
    assert_eq!(
        answers(&seeded(Origin::File(HOME.into()), Some(c"1"))).0,
        Some(b"~".to_vec())
    );
    assert_eq!(
        answers(&seeded(Origin::File(c"/".into()), Some(c"1"))).0,
        Some(b"/".to_vec())
    );
}

#[test]
fn the_dirtrim_value_is_strictly_decimal() {
    // D-619-6 on the 11-component origin. The parse accepts digits and an
    // optional `+`, and `010` is decimal 10 (the manual's base detection is
    // not what bash does — measured).
    let deep = Origin::File(c"/a/b/c/d/e/f/g/h/i/j/k".into());
    let full = b"/a/b/c/d/e/f/g/h/i/j/k".to_vec();
    for (value, want) in [
        (c"9", b".../c/d/e/f/g/h/i/j/k".as_slice()),
        (c"+2", b".../j/k"),
        (c"010", b".../b/c/d/e/f/g/h/i/j/k"),
    ] {
        assert_eq!(
            answers(&seeded(deep.clone(), Some(value))).0,
            Some(want.to_vec()),
            "measured parse row {value:?}"
        );
    }
    // No trim: `0`, non-numbers, overflow, and the pinned divergence that bash
    // tolerates surrounding whitespace (measured trims) while fdshell answers
    // no trim. Unset answers the same.
    for value in [
        c"0",
        c"abc",
        c"1e3",
        c"-1",
        c"999999999999999999999",
        c"",
        c" 2",
    ] {
        assert_eq!(
            answers(&seeded(deep.clone(), Some(value))).0,
            Some(full.clone()),
            "measured no-trim row {value:?}"
        );
    }
    assert_eq!(answers(&seeded(deep, None)).0, Some(full));
}

#[test]
fn separators_are_collapsed_before_the_ellipsis_is_counted() {
    // D-619-7 pinned against bash's measured rendering: `cd /tmp/.../y/` and
    // the `//` form both display normalised, and the rule counts those same
    // components.
    let trailing = Origin::File(c"/tmp/opencode/pdt/x/y/".into());
    assert_eq!(
        answers(&seeded(trailing.clone(), None)),
        (Some(b"/tmp/opencode/pdt/x/y".to_vec()), Some(b"y".to_vec()))
    );
    assert_eq!(
        answers(&seeded(trailing, Some(c"1"))).0,
        Some(b".../y".to_vec()),
        "the count is the collapsed count: 5 > 1 trims"
    );
    let doubled = Origin::File(c"/tmp/opencode//pdt/x//y".into());
    assert_eq!(
        answers(&seeded(doubled, None)),
        (Some(b"/tmp/opencode/pdt/x/y".to_vec()), Some(b"y".to_vec()))
    );
    // On the `~` branch the same: collapsed rest [tmp, pdt, x, y] at n = 2
    // passes `rest > n + 1` and trims.
    let tilde = Origin::File(c"/home/test//tmp/pdt/x//y".into());
    assert_eq!(
        answers(&seeded(tilde, Some(c"2"))).0,
        Some(b"~/.../x/y".to_vec())
    );
}

#[test]
fn no_dirstack_means_no_plus_or_minus_prefix() {
    // D-619-8: bash can prefix `\w` with `+`/`-` for dirstack entries; fdshell
    // has no dirstack, so the prefix never appears — the pinned plain answer,
    // handed to #623's README list.
    let cell = seeded(Origin::File(c"/tmp".into()), None);
    assert_eq!(
        answers(&cell),
        (Some(b"/tmp".to_vec()), Some(b"tmp".to_vec()))
    );
}
