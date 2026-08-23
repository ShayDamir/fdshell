#![allow(clippy::unwrap_used)]
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
