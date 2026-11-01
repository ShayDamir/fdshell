#![allow(clippy::unwrap_used)]
use super::*;
use sys::Origin;

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
/// The name lives in the function store and in the `ifs` field only, so a
/// predicate built from the three maps alone answers the other way.
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
/// panic (bash validates no `unset` operand at all).
#[test]
fn an_unbound_name_leaves_both_stores_alone() {
    let mut state = ShellState::new();
    let kept = ShortCStr::from(c"f");
    with_function(&mut state, &kept);
    state.unset_name(&ShortCStr::from(c"absent"));
    assert!(state.functions.contains_key(&kept), "nothing was removed");
    assert!(state.strings.is_empty());
    assert!(state.exports.is_empty());
}
