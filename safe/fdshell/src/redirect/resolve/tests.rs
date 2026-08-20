#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn out_of_range_export_to_is_actionable_error() {
    let mut state = ShellState::new();
    let fd = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    state.fds.insert(
        c"x".into(),
        crate::state::FdVar {
            fd,
            trace: sys::Trace::boundary(sys::Origin::Shell),
        },
    );
    let report = resolve_redirects(&[RedirectDef::var(i32::MAX, c"x")], &[], &state)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::FdNumberOutOfRange
    ));
}
