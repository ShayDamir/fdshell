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
    let redirects = vec![RedirectDef::here_string(c"$x", vec![false; 2])];
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
    let redirects = vec![RedirectDef::here_string(c"", Vec::new())];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"\n");
}

#[test]
fn here_string_reads_back_to_eof() {
    let c = cell();
    let redirects = vec![RedirectDef::here_string(c"abc", vec![false; 3])];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(n, 4);
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn here_string_quoted_escape_pair_keeps_the_backslash() {
    // `cat <<<"a\ b"`: the word's mask is all-quoted, so the POSIX 2.2.2 arm
    // keeps the backslash of a `\<char>` pair that is not `\$`, `\\` or `` \` ``
    // (bash prints `a\ b`; the empty mask printed `a b` — task #269).
    let c = cell();
    let redirects = vec![RedirectDef::here_string(c"a\\ b", vec![true; 4])];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(resolved.first().unwrap()).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"a\\ b\n");
}

#[test]
fn here_string_unquoted_escape_pair_folds_the_backslash() {
    // The same word unmasked folds its escape pair (POSIX 2.2.1), the shape of
    // `cat <<<a\ b`, where bash prints `a b` too.
    let c = cell();
    let redirects = vec![RedirectDef::here_string(c"a\\ b", Vec::new())];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(resolved.first().unwrap()).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"a b\n");
}

#[test]
fn here_doc_body_is_a_quoted_context() {
    // An expandable body is expanded as a quoted context (POSIX 2.2 "the
    // here-document is equivalent to double-quoting"): `a\ b` keeps its
    // backslash, and `$( … )` still runs (bash: `a\ b`).
    let c = cell();
    let redirects = vec![RedirectDef::here_doc(0, c"a\\ b\n", true)];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(resolved.first().unwrap()).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"a\\ b\n");
}

#[test]
fn here_doc_quoted_delimiter_body_stays_verbatim() {
    let c = cell();
    let redirects = vec![RedirectDef::here_doc(0, c"a\\ b $(echo hi)\n", false)];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let mut buf = [0u8; 32];
    let n = dup_of(resolved.first().unwrap()).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"a\\ b $(echo hi)\n");
}

#[test]
fn here_doc_writes_body_verbatim_without_newline() {
    let c = cell();
    let redirects = vec![RedirectDef::here_doc(0, c"ab\n", true)];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    assert!(matches!(r, Redirect::Dup { export_to: 0, .. }));
    let mut buf = [0u8; 16];
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(
        buf.get(..n).unwrap(),
        b"ab\n",
        "the body is written verbatim, with no extra newline"
    );
    assert_eq!(dup_of(r).read(&mut buf).unwrap(), 0);
}

#[test]
fn here_doc_empty_body_is_zero_bytes() {
    let c = cell();
    let redirects = vec![RedirectDef::here_doc(0, c"", false)];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    assert_eq!(dup_of(r).read(&mut buf).unwrap(), 0);
}

#[test]
fn here_doc_unquoted_delimiter_expands() {
    let c = cell();
    {
        let mut state = c.borrow_mut().unwrap();
        state.strings.insert(
            c"x".into(),
            sys::ImportedStr::new(c"world".into(), sys::Trace::boundary(sys::Origin::Shell)),
        );
    }
    let redirects = vec![RedirectDef::here_doc(0, c"hi $x\n", true)];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"hi world\n");
}

#[test]
fn here_doc_quoted_delimiter_is_literal() {
    let c = cell();
    {
        let mut state = c.borrow_mut().unwrap();
        state.strings.insert(
            c"x".into(),
            sys::ImportedStr::new(c"world".into(), sys::Trace::boundary(sys::Origin::Shell)),
        );
    }
    let redirects = vec![RedirectDef::here_doc(0, c"hi $x\n", false)];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let r = resolved.first().unwrap();
    let mut buf = [0u8; 16];
    let n = dup_of(r).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"hi $x\n");
}

/// The here-doc body is written to a memfd allocated **above** every export
/// target of the command, so `N<<word` never truncates the very descriptor it
/// feeds: the source of the fd-3 body sits above the highest target of the same
/// command list (LESSONS "redirect source fds must be allocated above the
/// command's highest target"; task #276 makes the target user-visible as `N`).
#[test]
fn numbered_here_doc_source_sits_above_the_export_targets() {
    let c = cell();
    let redirects = vec![
        RedirectDef::dup(5, 1),
        RedirectDef::here_doc(3, c"body\n", true),
    ];
    let resolved = resolve_redirects(&redirects, &[], &c).unwrap();
    let doc = resolved
        .iter()
        .find(|r| matches!(r, Redirect::Dup { export_to: 3, .. }))
        .unwrap();
    assert!(
        dup_of(doc).as_raw() > 5,
        "the source fd {} must be above the highest export target",
        dup_of(doc).as_raw()
    );
    let mut buf = [0u8; 16];
    let n = dup_of(doc).read(&mut buf).unwrap();
    assert_eq!(buf.get(..n).unwrap(), b"body\n");
}

/// The unprefixed form keeps its fd-0 target, and its source is a descriptor
/// above it (the same rule the `N<<word` form relies on).
#[test]
fn here_doc_source_is_above_its_zero_target() {
    let c = cell();
    let resolved = resolve_redirects(&[RedirectDef::here_doc(0, c"x\n", true)], &[], &c).unwrap();
    let doc = resolved.first().unwrap();
    assert!(dup_of(doc).as_raw() > 0);
}
