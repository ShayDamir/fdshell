#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn set_trap_stores_the_action_under_the_condition_number() {
    let mut state = ShellState::new();
    state.set_trap(1, TrapDisposition::Action(ShortCStr::from(c"echo hup")));
    assert_eq!(
        state.trap_at(1),
        TrapDisposition::Action(ShortCStr::from(c"echo hup"))
    );
}

#[test]
fn set_trap_replaces_the_action_of_an_earlier_registration() {
    let mut state = ShellState::new();
    state.set_trap(15, TrapDisposition::Action(ShortCStr::from(c"first")));
    state.set_trap(15, TrapDisposition::Action(ShortCStr::from(c"second")));
    assert_eq!(
        state.trap_at(15),
        TrapDisposition::Action(ShortCStr::from(c"second"))
    );
    assert_eq!(state.trap_list().len(), 1);
}

#[test]
fn an_empty_action_is_stored_as_ignore() {
    let mut state = ShellState::new();
    state.set_trap(2, TrapDisposition::Action(ShortCStr::from(c"")));
    assert_eq!(state.trap_at(2), TrapDisposition::Ignore);
}

#[test]
fn default_is_stored_as_no_entry() {
    let mut state = ShellState::new();
    state.set_trap(15, TrapDisposition::Default);
    assert!(state.traps.is_empty());
    assert_eq!(state.trap_at(15), TrapDisposition::Default);
    state.set_trap(15, TrapDisposition::Action(ShortCStr::from(c"bye")));
    state.reset_trap(15);
    assert!(state.traps.is_empty());
}

#[test]
fn trap_at_answers_default_for_an_unregistered_condition() {
    let state = ShellState::new();
    assert_eq!(state.trap_at(EXIT), TrapDisposition::Default);
    assert_eq!(state.trap_at(2), TrapDisposition::Default);
}

#[test]
fn the_exit_slot_leads_the_list_and_signals_follow_by_number() {
    let mut state = ShellState::new();
    state.set_trap(15, TrapDisposition::Action(ShortCStr::from(c"t")));
    state.set_trap(1, TrapDisposition::Action(ShortCStr::from(c"h")));
    state.set_trap(2, TrapDisposition::Action(ShortCStr::from(c"i")));
    state.set_trap(EXIT, TrapDisposition::Action(ShortCStr::from(c"e")));
    let order: Vec<i32> = state.trap_list().iter().map(|(cond, _)| *cond).collect();
    assert_eq!(order, [EXIT, 1, 2, 15]);
}

#[test]
fn the_list_carries_ignored_conditions_too() {
    let mut state = ShellState::new();
    state.set_trap(2, TrapDisposition::Ignore);
    state.set_trap(15, TrapDisposition::Action(ShortCStr::from(c"echo t")));
    let list = state.trap_list();
    assert_eq!(list.len(), 2);
    assert_eq!(list.first(), Some(&(2, &TrapDisposition::Ignore)));
    assert_eq!(
        list.get(1),
        Some(&(15, &TrapDisposition::Action(ShortCStr::from(c"echo t"))))
    );
}

#[test]
fn the_stored_action_is_the_exact_bytes() {
    let mut state = ShellState::new();
    let body = ShortCStr::from(c"echo 'a b' && echo \"it's the arc variant body\" && echo done");
    state.set_trap(15, TrapDisposition::Action(body));
    assert_eq!(
        state.trap_at(15),
        TrapDisposition::Action(ShortCStr::from(
            c"echo 'a b' && echo \"it's the arc variant body\" && echo done"
        ))
    );
}

#[test]
fn reset_drops_only_its_own_condition() {
    let mut state = ShellState::new();
    state.set_trap(EXIT, TrapDisposition::Action(ShortCStr::from(c"bye")));
    state.set_trap(15, TrapDisposition::Action(ShortCStr::from(c"t")));
    state.reset_trap(15);
    assert_eq!(
        state.trap_list(),
        [(EXIT, &TrapDisposition::Action(ShortCStr::from(c"bye")))]
    );
}

#[test]
fn a_fresh_shell_has_an_empty_table() {
    assert!(ShellState::new().traps.is_empty());
}
