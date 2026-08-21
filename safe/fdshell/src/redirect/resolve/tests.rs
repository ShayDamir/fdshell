#![allow(clippy::unwrap_used)]
use super::*;
use alloc::vec;

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

#[test]
fn out_of_range_export_to_is_actionable_error() {
    let c = cell();
    let fd = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    {
        let mut state = c.borrow_mut().unwrap();
        state.fds.insert(
            c"x".into(),
            crate::state::FdVar {
                fd,
                trace: sys::Trace::boundary(sys::Origin::Shell),
            },
        );
    }
    let report = resolve_redirects(&[RedirectDef::var(i32::MAX, c"x")], &[], &c)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::FdNumberOutOfRange
    ));
}

#[test]
fn unknown_var_source_names_the_missing_variable() {
    let c = cell();
    let report = resolve_redirects(&[RedirectDef::var(99, c"nope")], &[], &c)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::VarNotFound { var } if var.eq_bytes(b"nope")
    ));
}

#[test]
fn here_string_expands_word_and_appends_newline() {
    let c = cell();
    {
        let mut state = c.borrow_mut().unwrap();
        state.strings.insert(
            c"x".into(),
            sys::ImportedStr::new(c"hello".into(), sys::Trace::boundary(sys::Origin::Shell)),
        );
    }
    let redirects = vec![RedirectDef::here_string(c"$x")];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    assert_eq!(r.export_to, 0);
    let mut buf = [0u8; 16];
    let n = r.local.read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"hello\n");
}

#[test]
fn here_string_empty_word_is_bare_newline() {
    let c = cell();
    let redirects = vec![RedirectDef::here_string(c"")];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    let n = r.local.read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"\n");
}

#[test]
fn here_string_reads_back_to_eof() {
    let c = cell();
    let redirects = vec![RedirectDef::here_string(c"abc")];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    let n = r.local.read(&mut buf).unwrap();
    assert_eq!(n, 4);
    let n = r.local.read(&mut buf).unwrap();
    assert_eq!(n, 0);
}
