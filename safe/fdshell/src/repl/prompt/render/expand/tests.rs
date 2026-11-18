#![allow(clippy::unwrap_used)]

use alloc::format;
use alloc::vec::Vec;

use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::options::NOUNSET;
use crate::state::{FdVar, ShellState};

/// A state with the variables the pins read: `v` (the plain name), `hello`, and
/// `nope` deliberately absent. `HOME` is set so a leading `~` would move if this
/// surface had a tilde arm (it has none, measured).
fn cell() -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    for (name, value) in [(c"v", c"V"), (c"hello", c"world"), (c"HOME", c"/h/o/m/e")] {
        cell.borrow_mut()
            .unwrap()
            .strings
            .insert(ShortCStr::from(name), ImportedStr::shell(value.into()));
    }
    cell
}

/// Expand a run — the call the renderer makes per run, with the surface's mask.
fn expand(run: &[u8], cell: &ForkCell<ShellState>) -> Vec<u8> {
    super::expand_run(run, cell).unwrap()
}

#[test]
fn the_three_expansions_fire_on_a_prompt_run() {
    // The bash-measured rows: the plain and braced name forms and arithmetic
    // (`$( … )` and the backtick would need `substitute/tests.rs`'s `in_child`
    // shape, so those two live in the pty rows). The bash table's `${nope-x}` row
    // is not pinned here: fdshell's colon-less `-${word}` arm is a pre-existing
    // gap that answers empty on every surface, prompt included.
    let cell = cell();
    assert_eq!(expand(b"[$v|${v}|${hello}]", &cell), b"[V|V|world]");
    assert_eq!(expand(b"[a$((1+1))b]", &cell), b"[a2b]");
    assert_eq!(expand(b"[$hello]", &cell), b"[world]");
}

#[test]
fn quote_characters_are_data_and_do_not_protect() {
    // Measured: the quote bytes stay in the drawn prompt and nothing inside them
    // is protected — no tokenizer ran, so no mask bit and no quote span exists.
    let cell = cell();
    assert_eq!(expand(b"a\"b$v\"c", &cell), b"a\"bV\"c");
    assert_eq!(expand(b"\"$v\"", &cell), b"\"V\"");
    assert_eq!(expand(b"'$v'", &cell), b"'V'");
}

#[test]
fn the_backslash_quotes_only_the_special_set() {
    // Measured: `\` before `$ \`" ' \ is a quote operator (removed); before
    // anything else it keeps its own byte. The combined row is the one that
    // separates the two arms: its second backslash is preceded by a byte of the
    // set and followed by one that is not.
    let cell = cell();
    assert_eq!(expand(b"a\\\"b", &cell), b"a\"b");
    assert_eq!(expand(b"a\\\\b", &cell), b"a\\b");
    assert_eq!(expand(b"\\qX", &cell), b"\\qX");
    assert_eq!(expand(b"a\\q\\\"b", &cell), b"a\\q\"b");
}

#[test]
fn no_tilde_no_globbing_no_splitting_no_separators() {
    // Measured, one row per fact: `~ ` and `* ` stay themselves, and the bytes
    // the word pipeline would act on are kept verbatim.
    let cell = cell();
    assert_eq!(expand(b"~ ", &cell), b"~ ");
    assert_eq!(expand(b"* ", &cell), b"* ");
    assert_eq!(expand(b"a  b ", &cell), b"a  b ");
    assert_eq!(expand(b"a;b|c", &cell), b"a;b|c");
}

#[test]
fn the_percent_arm_fires_on_this_surface() {
    // D-620-5: `%name` is fdshell's own syntax and every surface except the
    // redirect target fires it, so a bound name draws its fd number (bash draws
    // `%CWD` literally — the divergence #623 documents) and an unbound one
    // writes a literal `%` plus the name.
    let cell = cell();
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    let number = dev_null.as_raw();
    cell.borrow_mut().unwrap().fds.insert(
        ShortCStr::from(c"testfd"),
        FdVar {
            fd: dev_null,
            trace: sys::Trace::boundary(sys::Origin::Shell),
        },
    );
    assert_eq!(
        expand(b"[a%testfd|%CWD]", &cell),
        format!("[a{number}|%CWD]").as_bytes()
    );
}

#[test]
fn the_positional_join_reproduces_the_measured_row() {
    // `set -- a b`, `IFS=,`, `[${*}|${@}]` → `[a,b|a b] ` (bash 5.3.9 measured):
    // `$*` joins with the first `IFS` byte, `$@` **space**-joins. LESSONS: "`$@`
    // has two join rules" — the prompt surface is the surface that already had
    // this rule, so it is the one the prompt entry point names.
    let cell = cell();
    for value in [c"a", c"b"] {
        cell.borrow_mut()
            .unwrap()
            .positional
            .push_back(ImportedStr::shell(value.into()));
    }
    cell.borrow_mut().unwrap().ifs = ShortCStr::from(c",");
    assert_eq!(expand(b"[${*}|${@}]", &cell), b"[a,b|a b]");
}

#[test]
fn a_nul_splits_the_run_and_is_pasted() {
    // Only `\000`/`\x00` put a NUL into a run. Each side is expanded, the NUL is
    // pasted — the engine's input type is NUL-free, so the split is the whole
    // mechanism (no new error variant).
    let cell = cell();
    assert_eq!(expand(b"[a\0b$v]", &cell), b"[a\0bV]");
}

#[test]
fn nounset_reports_and_the_runner_keeps_the_literal_run() {
    // D-620-8: bash prints its own error on stderr, draws the prompt as the
    // unexpanded text and keeps prompting. The report is the stage's answer; the
    // renderer pastes the run's bytes and the REPL keeps going (the pty row is
    // the end-to-end half).
    let cell = cell();
    cell.borrow_mut().unwrap().options |= NOUNSET;
    let report = super::expand_run(b"[a$nope]", &cell).unwrap_err();
    assert!(matches!(
        report.current_context(),
        ResolveError::UnboundVariable { .. }
    ));
}
