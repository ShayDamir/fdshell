#![allow(clippy::unwrap_used)]
use super::{pop_frame, push_frame};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ShortCStr};

use crate::error::cmd::CmdError;
use crate::state::ShellState;

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

fn set(state: &mut ShellState, name: &ShortCStr, value: &ShortCStr) {
    state.set_var(name.clone(), ImportedStr::shell(value.clone()));
}

fn strings(state: &ShellState, name: &ShortCStr) -> Option<ShortCStr> {
    state.strings.get(name).map(|v| v.value.clone())
}

fn exports(state: &ShellState, name: &ShortCStr) -> Option<ShortCStr> {
    state.exports.get(name).map(|v| v.value.clone())
}

fn shadow(state: &mut ShellState, name: &ShortCStr) {
    state.shadow(name).unwrap();
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

#[test]
fn begin_shadow_end_restores_strings() {
    let mut state = ShellState::new();
    set(&mut state, &s(b"v"), &s(b"pre"));
    state.begin_frame();
    shadow(&mut state, &s(b"v"));
    set(&mut state, &s(b"v"), &s(b"1"));
    assert_eq!(strings(&state, &s(b"v")), Some(s(b"1")));
    state.end_frame();
    assert_eq!(strings(&state, &s(b"v")), Some(s(b"pre")));
}

#[test]
fn first_touch_keeps_the_caller_value() {
    let mut state = ShellState::new();
    set(&mut state, &s(b"v"), &s(b"pre"));
    state.begin_frame();
    shadow(&mut state, &s(b"v"));
    set(&mut state, &s(b"v"), &s(b"1"));
    shadow(&mut state, &s(b"v"));
    set(&mut state, &s(b"v"), &s(b"2"));
    state.end_frame();
    assert_eq!(strings(&state, &s(b"v")), Some(s(b"pre")));
}

#[test]
fn declare_form_removes_the_value_and_restores_it() {
    let mut state = ShellState::new();
    set(&mut state, &s(b"v"), &s(b"pre"));
    state.begin_frame();
    shadow(&mut state, &s(b"v"));
    // The declare form (`local v`) leaves the name unset for the call.
    let _ = state.strings.remove(&s(b"v"));
    assert_eq!(strings(&state, &s(b"v")), None);
    state.end_frame();
    assert_eq!(strings(&state, &s(b"v")), Some(s(b"pre")));
}

#[test]
fn exports_are_restored() {
    let mut state = ShellState::new();
    state.exports.insert(s(b"E"), ImportedStr::shell(s(b"env")));
    state.begin_frame();
    shadow(&mut state, &s(b"E"));
    state.exports.insert(s(b"E"), ImportedStr::shell(s(b"loc")));
    state.end_frame();
    assert_eq!(exports(&state, &s(b"E")), Some(s(b"env")));
    assert_eq!(state.exports.len(), 1);
}

#[test]
fn ifs_restored_after_local_ifs() {
    let mut state = ShellState::new();
    let caller_ifs = state.ifs.clone();
    state.begin_frame();
    shadow(&mut state, &s(b"IFS"));
    set(&mut state, &s(b"IFS"), &s(b":"));
    assert_eq!(state.ifs, s(b":"));
    state.end_frame();
    assert_eq!(state.ifs, caller_ifs);
}

#[test]
fn nested_frames_unwind_in_order() {
    let mut state = ShellState::new();
    set(&mut state, &s(b"z"), &s(b"top"));
    state.begin_frame();
    shadow(&mut state, &s(b"z"));
    set(&mut state, &s(b"z"), &s(b"outer"));
    state.begin_frame();
    shadow(&mut state, &s(b"z"));
    set(&mut state, &s(b"z"), &s(b"inner"));
    state.end_frame();
    assert_eq!(strings(&state, &s(b"z")), Some(s(b"outer")));
    state.end_frame();
    assert_eq!(strings(&state, &s(b"z")), Some(s(b"top")));
}

#[test]
fn end_frame_without_frame_is_a_noop() {
    let mut state = ShellState::new();
    set(&mut state, &s(b"v"), &s(b"pre"));
    state.end_frame();
    assert_eq!(strings(&state, &s(b"v")), Some(s(b"pre")));
    assert!(!state.in_frame());
    assert!(state.local_names().is_empty());
}

#[test]
fn shadow_without_frame_is_never() {
    let mut state = ShellState::new();
    match state.shadow(&s(b"v")) {
        Ok(()) => panic!("shadow outside a frame must fail"),
        Err(report) => assert!(matches!(report.current_context(), CmdError::Never)),
    }
}

#[test]
fn in_frame_tracks_the_frame_stack() {
    let mut state = ShellState::new();
    assert!(!state.in_frame());
    state.begin_frame();
    assert!(state.in_frame());
    state.begin_frame();
    shadow(&mut state, &s(b"v"));
    shadow(&mut state, &s(b"w"));
    // `local_names` reports the innermost frame only.
    let names = state.local_names();
    assert_eq!(names.len(), 2);
    assert!(names.iter().any(|n| n.eq_bytes(b"v")));
    state.end_frame();
    assert!(state.local_names().is_empty());
    state.end_frame();
    assert!(!state.in_frame());
}

#[test]
fn push_pop_frame_drive_the_state_through_the_cell() {
    let cell = make_cell();
    set(&mut cell.borrow_mut().unwrap(), &s(b"v"), &s(b"pre"));
    push_frame(&cell).unwrap();
    let mut state = cell.borrow_mut().unwrap();
    shadow(&mut state, &s(b"v"));
    set(&mut state, &s(b"v"), &s(b"1"));
    drop(state);
    pop_frame(&cell).unwrap();
    assert_eq!(strings(&cell.borrow().unwrap(), &s(b"v")), Some(s(b"pre")));
}
