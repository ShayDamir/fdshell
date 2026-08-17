#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn shift_removes_n_positional() {
    let mut state = ShellState::new();
    state.positional.push_back(c"a".into());
    state.positional.push_back(c"b".into());
    state.positional.push_back(c"c".into());
    state.shift(2);
    assert_eq!(state.positional.len(), 1);
    assert_eq!(state.positional.front().unwrap().as_bytes().unwrap(), b"c");
}
