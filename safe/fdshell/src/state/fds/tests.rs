#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn fd_var_answers_none_for_a_name_nothing_holds() {
    let state = ShellState::new();
    assert!(state.fd_var(&ShortCStr::from(c"nothing")).is_none());
}

#[test]
fn fd_var_answers_the_descriptor_it_was_given() {
    let mut state = ShellState::new();
    let fd = sys::memfd::memfd_create().unwrap();
    let raw = fd.as_raw();
    let var = FdVar {
        fd,
        trace: sys::Trace::boundary(sys::Origin::Shell),
    };
    state.set_fd_var(ShortCStr::from(c"a"), var);
    let found = state.fd_var(&ShortCStr::from(c"a")).unwrap();
    assert_eq!(found.fd.as_raw(), raw);
}
