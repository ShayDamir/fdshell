#![allow(clippy::unwrap_used)]
use alloc::vec;
use alloc::vec::Vec;

use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, Position, ScriptText, ShortCStr, Trace};

use crate::parse::{BuiltinPrefix, CommandLine};
use crate::state::ShellState;

use super::{EnvAssign, apply, apply_child, expand, restore};

fn cell_with(pairs: &[(&str, &str)]) -> ForkCell<ShellState> {
    let mut state = ShellState::new();
    for (name, value) in pairs {
        let n = ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap();
        let v = ShortCStr::from_vec(value.as_bytes().to_vec()).unwrap();
        state.set_var(n, ImportedStr::new(v, Trace::boundary(Origin::Shell)));
    }
    ForkCell::new(state)
}

fn assign(name: &str, value: &str) -> EnvAssign {
    EnvAssign {
        name: ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap(),
        value: ImportedStr::new(
            ShortCStr::from_vec(value.as_bytes().to_vec()).unwrap(),
            Trace::boundary(Origin::Shell),
        ),
    }
}

fn get(cell: &ForkCell<ShellState>, name: &str, map: &str) -> Option<ShortCStr> {
    let state = cell.borrow().unwrap();
    let key = ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap();
    let entry = if map == "strings" {
        state.strings.get(&key).cloned()
    } else {
        state.exports.get(&key).cloned()
    };
    entry.map(|i| i.value)
}

#[test]
fn apply_sets_strings_and_exports() {
    let cell = cell_with(&[]);
    let save = apply(&[assign("FOO", "bar")], &cell).unwrap();
    assert!(save.is_some());
    assert_eq!(get(&cell, "FOO", "strings"), Some(c"bar".into()));
    assert_eq!(get(&cell, "FOO", "exports"), Some(c"bar".into()));
}

#[test]
fn restore_puts_original_value_back() {
    let cell = cell_with(&[("FOO", "old")]);
    let save = apply(&[assign("FOO", "new")], &cell).unwrap();
    restore(save, &cell).unwrap();
    assert_eq!(get(&cell, "FOO", "strings"), Some(c"old".into()));
    assert!(get(&cell, "FOO", "exports").is_none());
}

#[test]
fn restore_puts_back_previous_export() {
    let cell = cell_with(&[("FOO", "old")]);
    {
        let v = cell
            .borrow()
            .unwrap()
            .strings
            .get::<ShortCStr>(&c"FOO".into())
            .unwrap()
            .clone();
        cell.borrow_mut().unwrap().exports.insert(c"FOO".into(), v);
    }
    let save = apply(&[assign("FOO", "new")], &cell).unwrap();
    restore(save, &cell).unwrap();
    assert_eq!(get(&cell, "FOO", "strings"), Some(c"old".into()));
    assert_eq!(get(&cell, "FOO", "exports"), Some(c"old".into()));
}

#[test]
fn restore_removes_var_that_was_not_set() {
    let cell = cell_with(&[]);
    let save = apply(&[assign("NEW", "1")], &cell).unwrap();
    restore(save, &cell).unwrap();
    assert!(get(&cell, "NEW", "strings").is_none());
    assert!(get(&cell, "NEW", "exports").is_none());
}

#[test]
fn restore_saves_first_touch_only() {
    let cell = cell_with(&[("FOO", "orig")]);
    let save = apply(&[assign("FOO", "1"), assign("FOO", "2")], &cell).unwrap();
    assert_eq!(get(&cell, "FOO", "strings"), Some(c"2".into()));
    restore(save, &cell).unwrap();
    assert_eq!(get(&cell, "FOO", "strings"), Some(c"orig".into()));
}

#[test]
fn apply_syncs_ifs_and_restore_reverts_it() {
    let cell = cell_with(&[]);
    let save = apply(&[assign("IFS", ":")], &cell).unwrap();
    assert_eq!(cell.borrow().unwrap().ifs.as_bytes().unwrap_or(&[]), b":");
    restore(save, &cell).unwrap();
    assert_eq!(
        cell.borrow().unwrap().ifs.as_bytes().unwrap_or(&[]),
        b" \t\n"
    );
    // IFS was not seeded in `strings`, so it must not leak there on restore.
    assert!(get(&cell, "IFS", "strings").is_none());
}

#[test]
fn empty_env_applies_nothing_and_restore_is_a_noop() {
    let cell = cell_with(&[("FOO", "old")]);
    let save = apply(&Vec::new(), &cell).unwrap();
    assert!(save.is_none());
    restore(None, &cell).unwrap();
    assert_eq!(get(&cell, "FOO", "strings"), Some(c"old".into()));
}

#[test]
fn apply_child_sets_strings_and_exports() {
    let cell = cell_with(&[]);
    apply_child(&[assign("FOO", "bar")], &cell).unwrap();
    assert_eq!(get(&cell, "FOO", "strings"), Some(c"bar".into()));
    assert_eq!(get(&cell, "FOO", "exports"), Some(c"bar".into()));
}

#[test]
fn expand_expands_value_words_against_state() {
    let cell = cell_with(&[("A", "1")]);
    let cl = CommandLine {
        prefix: BuiltinPrefix::None,
        command: ShortCStr::from_vec(b"cmd".to_vec()).unwrap(),
        command_mask: vec![],
        env_assigns: vec![(
            ShortCStr::from_vec(b"FOO".to_vec()).unwrap(),
            ShortCStr::from_vec(b"$A".to_vec()).unwrap(),
        )],
        args: vec![],
        args_mask: vec![],
        args_quoted: vec![],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    };
    let text = ScriptText::new(
        ShortCStr::from_vec(b"FOO=$A cmd".to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let env = expand(&cl, &text, &cell).unwrap();
    assert_eq!(env.len(), 1);
    let applied = apply(&env, &cell).unwrap();
    assert_eq!(get(&cell, "FOO", "strings"), Some(c"1".into()));
    restore(applied, &cell).unwrap();
}
