#![allow(clippy::unwrap_used)]
use super::*;
use alloc::vec;

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn dup_of(r: &Redirect) -> &LocalFd {
    match r {
        Redirect::Dup { local, .. } => local,
        Redirect::Close { .. } => panic!("expected Dup"),
    }
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
fn out_of_range_path_redirect_is_actionable_error() {
    let c = cell();
    let report = resolve_redirects(&[RedirectDef::write_path(i32::MAX, c"f")], &[], &c)
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
fn dup_redirect_clones_open_fd() {
    let c = cell();
    let resolved = resolve_redirects(&[RedirectDef::dup(7, 1)], &[], &c).unwrap();
    let r = resolved.first().unwrap();
    assert!(matches!(r, Redirect::Dup { export_to: 7, .. }));
    assert!(dup_of(r).verify().is_ok());
}

#[test]
fn dup_redirect_closed_source_is_actionable_error() {
    let c = cell();
    let report = resolve_redirects(&[RedirectDef::dup(7, 999)], &[], &c)
        .err()
        .unwrap();
    assert!(matches!(
        report.current_context(),
        OpenRedirectError::FdNotOpen { n: 999 }
    ));
}

#[test]
fn close_redirect_closes_the_fd() {
    let c = cell();
    let (rd, _wr) = sys::pipe::pipe2(0).unwrap();
    let n = rd.as_raw();
    let resolved = resolve_redirects(&[RedirectDef::close(n)], &[], &c).unwrap();
    assert!(matches!(
        resolved.first().unwrap(),
        Redirect::Close { export_to: _n } if *_n == n
    ));
    resolved.first().unwrap().export().unwrap();
    let probe = match sys::ImportedFd::from_number(n) {
        Ok(_) => panic!("expected closed fd to fail validation"),
        Err(e) => e,
    };
    assert!(matches!(
        probe.current_context(),
        sys::ImportedFdError::GetFlags
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
    assert!(matches!(r, Redirect::Dup { export_to: 0, .. }));
    let mut buf = [0u8; 16];
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"hello\n");
}

#[test]
fn here_string_empty_word_is_bare_newline() {
    let c = cell();
    let redirects = vec![RedirectDef::here_string(c"")];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"\n");
}

#[test]
fn here_string_reads_back_to_eof() {
    let c = cell();
    let redirects = vec![RedirectDef::here_string(c"abc")];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(n, 4);
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(n, 0);
}
