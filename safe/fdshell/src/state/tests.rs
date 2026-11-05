#![allow(clippy::unwrap_used)]
use alloc::vec;

use sys::Origin;
use sys::fork_cell::ForkCell;

use crate::command_word::expand;
use crate::mask::MaskBit;
use crate::parse::{BuiltinPrefix, CommandLine};

use super::ctor::{seeded_environ, seeded_strings};
use super::*;

#[test]
fn shift_removes_n_positional() {
    let mut state = ShellState::new();
    state.positional.push_back(ImportedStr::shell(c"a".into()));
    state.positional.push_back(ImportedStr::shell(c"b".into()));
    state.positional.push_back(ImportedStr::shell(c"c".into()));
    state.shift(2);
    assert_eq!(state.positional.len(), 1);
    assert_eq!(
        state.positional.front().unwrap().value.as_bytes().unwrap(),
        b"c"
    );
}

#[test]
fn program_name_defaults_to_fdshell() {
    let state = ShellState::new();
    assert_eq!(
        state.program_name.value.as_bytes().unwrap(),
        b"fdshell".as_slice()
    );
    assert!(matches!(state.program_name.trace.origin, Origin::Shell));
    assert!(state.positional.is_empty());
}

#[test]
fn set_program_name_replaces_it() {
    let mut state = ShellState::new();
    state.set_program_name(ImportedStr::shell(c"s.sh".into()));
    assert_eq!(
        state.program_name.value.as_bytes().unwrap(),
        b"s.sh".as_slice()
    );
}

#[test]
fn set_last_arg_stores_underscore() {
    let mut state = ShellState::new();
    state.set_last_arg(c"w".into());
    let key: ShortCStr = c"_".into();
    let val = state.strings.get(&key).unwrap();
    assert_eq!(val.value.as_bytes().unwrap(), b"w");
}

#[test]
fn clear_last_arg_stores_empty() {
    let mut state = ShellState::new();
    state.set_last_arg(c"w".into());
    state.clear_last_arg();
    let key: ShortCStr = c"_".into();
    let val = state.strings.get(&key).unwrap();
    assert_eq!(val.value.as_bytes().unwrap(), b"");
}

#[test]
fn last_arg_updates_are_gated_inside_eval_frame() {
    let mut state = ShellState::new();
    state.begin_eval();
    assert_eq!(state.eval_depth, 1);
    state.set_last_arg(c"x".into());
    state.clear_last_arg();
    let key: ShortCStr = c"_".into();
    assert!(!state.strings.contains_key(&key));
    state.end_eval();
    assert_eq!(state.eval_depth, 0);
}

#[test]
fn last_arg_updates_resume_after_frame_ends() {
    let mut state = ShellState::new();
    state.begin_eval();
    state.end_eval();
    state.set_last_arg(c"y".into());
    let key: ShortCStr = c"_".into();
    let val = state.strings.get(&key).unwrap();
    assert_eq!(val.value.as_bytes().unwrap(), b"y");
}

#[test]
fn end_eval_saturates_at_zero() {
    let mut state = ShellState::new();
    state.end_eval();
    assert_eq!(state.eval_depth, 0);
}

/// Seeds the function store, which every `unset_name` pin below needs: the
/// namespace choice is only observable from a name that lives there (task #386).
fn with_function(state: &mut ShellState, name: &ShortCStr) {
    state.functions.insert(name.clone(), ShortCStr::from(c":"));
}

/// A name in both stores is a variable first: `unset_name` empties the variable
/// store and leaves the function alone (bash's priority, measured).
#[test]
fn a_name_in_both_stores_loses_the_variable_and_keeps_the_function() {
    let mut state = ShellState::new();
    let name = ShortCStr::from(c"f");
    state.set_var(name.clone(), ImportedStr::shell(c"V".into()));
    with_function(&mut state, &name);
    state.unset_name(&name);
    assert!(
        !state.strings.contains_key(&name),
        "the variable is what goes"
    );
    assert!(state.functions.contains_key(&name), "the function stays");
}

/// A name no variable store holds is a function, and `unset_name` removes it —
/// the row that made this a bug, and the pin killing an "always variable"
/// predicate.
#[test]
fn a_name_only_the_function_store_has_is_removed() {
    let mut state = ShellState::new();
    let name = ShortCStr::from(c"f");
    with_function(&mut state, &name);
    state.unset_name(&name);
    assert!(!state.functions.contains_key(&name));
}

/// The export attribute is one of the stores the delete path owns, so a name
/// living there is a variable and its function stays.
#[test]
fn the_export_attribute_is_a_variable_so_the_function_stays() {
    let mut state = ShellState::new();
    let name = ShortCStr::from(c"f");
    state
        .exports
        .insert(name.clone(), ImportedStr::shell(c"V".into()));
    with_function(&mut state, &name);
    state.unset_name(&name);
    assert!(!state.exports.contains_key(&name), "the export goes");
    assert!(state.functions.contains_key(&name));
}

/// The inherited `environ` row is one of the stores the delete path owns, so a
/// name inherited from the parent is a variable and its function stays.
#[test]
fn the_inherited_environment_is_a_variable_so_the_function_stays() {
    let mut state = ShellState::new();
    let name = ShortCStr::from(c"f");
    state.environ.push((name.clone(), ShortCStr::from(c"V")));
    with_function(&mut state, &name);
    state.unset_name(&name);
    assert!(
        !state.environ.iter().any(|(k, _)| *k == name),
        "the row goes"
    );
    assert!(state.functions.contains_key(&name));
}

/// `IFS` is a variable for `unset` too — the single-name sync is part of the
/// predicate, so a function named `IFS` survives `unset IFS` (bash, measured).
/// The startup seed puts the name in `strings`, so the map half of the predicate
/// reaches it here; the explicit `IFS` clause is what answers on a **second**
/// `unset IFS`, after the seed itself is gone (task #512).
#[test]
fn ifs_is_a_variable_so_a_function_named_ifs_stays() {
    let mut state = ShellState::new();
    let name = ShortCStr::from(c"IFS");
    state.ifs = ShortCStr::from(c",");
    with_function(&mut state, &name);
    state.unset_name(&name);
    assert_eq!(
        state.ifs,
        ShellState::default_ifs(),
        "the IFS sync ran, so the variable branch was taken"
    );
    assert!(state.functions.contains_key(&name));
}

/// A name in neither store is a silent no-op in both directions: no removal, no
/// panic (bash validates no `unset` operand at all). Both stores are named by
/// entry, not by size — the `IFS` seed is in `strings` from startup (task #388),
/// so a size assertion would be about the seed, not about this no-op.
#[test]
fn an_unbound_name_leaves_both_stores_alone() {
    let mut state = ShellState::new();
    let kept = ShortCStr::from(c"f");
    with_function(&mut state, &kept);
    let absent = ShortCStr::from(c"absent");
    state.unset_name(&absent);
    assert!(state.functions.contains_key(&kept), "nothing was removed");
    assert!(!state.strings.contains_key(&absent), "nothing was removed");
    assert!(!state.exports.contains_key(&absent), "nothing was removed");
}

/// The fresh shell **sets `IFS` to its default value** (POSIX XCU 2.5.1, bash
/// 5.3.9), so `$IFS` answers the default split set bytes whether or not the
/// parent exported the name, and the value's provenance is the shell, not the
/// environment. Without this seed `$IFS` answers empty in a clean environment
/// and the parent's bytes in an inheriting one.
#[test]
fn the_string_store_is_seeded_with_the_default_ifs() {
    let state = ShellState::new();
    let seed = state.strings.get(&ShortCStr::from(c"IFS")).unwrap();
    assert!(
        seed.value.eq_bytes(b" \t\n"),
        "the default split set, byte for byte"
    );
    assert!(matches!(seed.trace.origin, Origin::Shell));
    assert!(seeded_strings().contains_key(&ShortCStr::from(c"IFS")));
}

/// The snapshot's `IFS` row keeps its place (the parent's export attribute
/// survives, and bash passes it on) but carries the **default** value; every
/// other row is the parent's. A snapshot without an `IFS` row is returned
/// unchanged — no row means no export (bash measures `grep -c` 0), and the seed
/// of `strings` alone answers `$IFS`.
#[test]
fn the_environ_seed_rewrites_only_the_ifs_row() {
    let snapshot = vec![
        (ShortCStr::from(c"IFS"), ShortCStr::from(c",")),
        (ShortCStr::from(c"PATH"), ShortCStr::from(c"/bin")),
    ];
    let seeded = seeded_environ(snapshot);
    assert_eq!(seeded.len(), 2, "the row stays, only its value moves");
    assert!(value_of(&seeded, b"IFS").unwrap().eq_bytes(b" \t\n"));
    assert!(
        value_of(&seeded, b"PATH").unwrap().eq_bytes(b"/bin"),
        "an unrelated inherited row keeps its value"
    );
    let no_row = seeded_environ(vec![(ShortCStr::from(c"PATH"), ShortCStr::from(c"/bin"))]);
    assert_eq!(no_row.len(), 1, "nothing is added");
    assert!(value_of(&no_row, b"PATH").unwrap().eq_bytes(b"/bin"));
}

fn value_of(rows: &[(ShortCStr, ShortCStr)], name: &[u8]) -> Option<ShortCStr> {
    rows.iter()
        .find(|(key, _)| key.eq_bytes(name))
        .map(|(_, value)| value.clone())
}

/// A command line whose word 0 is raw (unquoted) text and which has no other word
/// — the shape whose expansion produces no field (task #426).
fn word0(command: &str) -> CommandLine {
    let word = ShortCStr::from_vec(command.as_bytes().to_vec()).unwrap();
    CommandLine {
        prefix: BuiltinPrefix::None,
        command_mask: vec![MaskBit::Unquoted; command.len()],
        command_span: vec![],
        command_quoted: false,
        command: word,
        env_assigns: vec![],
        args: vec![],
        args_mask: vec![],
        args_span: vec![],
        args_quoted: vec![],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

/// The per-command scratch (task #426). The reap site's value survives the
/// expansion of the word that **ran** the substitution — a `$(false)` in word-0
/// position answers 1 — and `expand`'s top clears both fields, so a substitution
/// recorded for one command never answers for the next: bash's rule is the status
/// of the last substitution performed, 0 when none ran (`$(false); $unset;
/// echo rc=$?` → `rc=0`, measured). Both directions are pinned: the marker is the
/// word's own, the status is the word's own.
#[test]
fn the_word_scratch_is_per_command() {
    let cell = ForkCell::new(ShellState::new());
    expand(&word0("$(false)"), &cell).unwrap();
    assert!(cell.borrow().unwrap().word0_vanished, "no field = no name");
    assert_eq!(
        recorded(&cell),
        Some(1),
        "the value recorded at the reap site survives to the arm"
    );

    // A word that vanishes **without** running a substitution of its own: the
    // marker is set, the recorded status from the earlier word is gone.
    expand(&word0("$unset"), &cell).unwrap();
    assert!(cell.borrow().unwrap().word0_vanished);
    assert_eq!(
        recorded(&cell),
        None,
        "the earlier word's substitution leaked"
    );

    // And a word whose own substitution ran is answered by it again (bash 0).
    expand(&word0("$(true)"), &cell).unwrap();
    assert_eq!(recorded(&cell), Some(0));
}

fn recorded(cell: &ForkCell<ShellState>) -> Option<i32> {
    cell.borrow()
        .unwrap()
        .last_substitution
        .as_ref()
        .map(|s| s.exit_code())
}
