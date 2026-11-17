use super::*;

#[test]
fn a_flag_set_alone_answers_true() {
    let mut state = ShellState::new();
    state.options |= options::STRICT;
    assert!(state.option_on(options::STRICT));
    assert!(state.strict());
}

#[test]
fn a_flag_off_with_another_bit_set_answers_false() {
    let mut state = ShellState::new();
    state.options |= options::NOUNSET;
    assert!(!state.option_on(options::STRICT));
    assert!(!state.strict());
}
