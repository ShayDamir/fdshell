#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use error_stack::Report;
use sys::{Origin, ShortCStr, Trace};

use crate::state::{FdVar, ShellState};

use super::emit::{kind, line};
use super::flags;
use super::parse::{Target, statx_parse};
use super::{dirfd, handle_statx};

fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr], &[ShortCStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    f(&refs, &origs)
}

fn is_invalid(e: &Report<BuiltinError>, what: &'static str) -> bool {
    matches!(e.current_context(), BuiltinError::InvalidArgument(s) if *s == what)
}

fn state_with_memfd() -> ShellState {
    let mut state = ShellState::new();
    let fd = sys::memfd::memfd_create().unwrap();
    state.fds.insert(
        c"f".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    state
}

#[test]
fn path_form_defaults() {
    with_refs(&["a.txt"], |refs, origs| {
        match statx_parse(refs, origs).unwrap() {
            Target::Path {
                path,
                dir: None,
                nofollow: false,
            } => assert_eq!(path, c"a.txt"),
            other => panic!("unexpected {other:?}"),
        }
    });
}

#[test]
fn path_form_dir_and_nofollow() {
    with_refs(
        &["a.txt", "--dir", "%d", "--nofollow"],
        |refs, origs| match statx_parse(refs, origs).unwrap() {
            Target::Path {
                dir: Some(d),
                nofollow: true,
                ..
            } => {
                assert_eq!(d.as_bytes().unwrap(), b"d")
            }
            other => panic!("unexpected {other:?}"),
        },
    );
    with_refs(&["a.txt", "--dir=%d"], |refs, origs| {
        assert!(matches!(
            statx_parse(refs, origs).unwrap(),
            Target::Path {
                dir: Some(_),
                nofollow: false,
                ..
            }
        ));
    });
}

#[test]
fn fd_form() {
    with_refs(&["%f", "--nofollow"], |refs, origs| {
        match statx_parse(refs, origs).unwrap() {
            Target::Fd {
                var,
                nofollow: true,
            } => assert_eq!(var.as_bytes().unwrap(), b"f"),
            other => panic!("unexpected {other:?}"),
        }
    });
}

#[test]
fn fd_form_rejects_dir() {
    with_refs(&["%f", "--dir", "%d"], |refs, origs| {
        let e = statx_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "--dir"));
    });
}

#[test]
fn unknown_flag_rejected() {
    for args in [
        &["a.txt", "--bogus"][..],
        &["%f", "extra"][..],
        &["a.txt", "b.txt"][..],
    ] {
        with_refs(args, |refs, origs| {
            let e = statx_parse(refs, origs).unwrap_err();
            assert!(is_invalid(&e, "flag"), "{args:?}");
        });
    }
}

#[test]
fn dir_var_must_be_prefixed() {
    for args in [&["a.txt", "--dir", "d"][..], &["a.txt", "--dir", "%"][..]] {
        with_refs(args, |refs, origs| {
            let e = statx_parse(refs, origs).unwrap_err();
            assert!(is_invalid(&e, "dir var"), "{args:?}");
        });
    }
}

#[test]
fn duplicate_dir_rejected() {
    with_refs(&["a.txt", "--dir", "%d", "--dir", "%e"], |refs, origs| {
        let e = statx_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "--dir"));
    });
}

#[test]
fn double_percent_var_rejected() {
    with_refs(&["%%"], |refs, origs| {
        let e = statx_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "fd var"));
    });
}

#[test]
fn bare_percent_resolves_to_unset_var() {
    // An empty var name parses; it simply resolves to nothing.
    let state = state_with_memfd();
    with_refs(&["%"], |refs, origs| {
        let e = handle_statx(c"statx".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

#[test]
fn missing_path_errors() {
    with_refs(&[], |refs, origs| {
        let e = statx_parse(refs, origs).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("path")
        ));
    });
}

#[test]
fn empty_path_rejected() {
    with_refs(&[""], |refs, origs| {
        let e = statx_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "path"));
    });
}

#[test]
fn nofollow_takes_no_value() {
    with_refs(&["a.txt", "--nofollow=x"], |refs, origs| {
        let e = statx_parse(refs, origs).unwrap_err();
        assert!(is_invalid(&e, "--nofollow"));
    });
}

#[test]
fn help() {
    with_refs(&["--help"], |refs, origs| {
        let e = statx_parse(refs, origs).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Help));
    });
}

#[test]
fn kind_covers_all_file_types() {
    use sys::stat::*;
    assert_eq!(kind(S_IFREG | 0o644), "file");
    assert_eq!(kind(S_IFDIR | 0o755), "dir");
    assert_eq!(kind(S_IFLNK | 0o777), "symlink");
    assert_eq!(kind(S_IFIFO | 0o600), "fifo");
    assert_eq!(kind(S_IFCHR | 0o644), "char");
    assert_eq!(kind(S_IFBLK | 0o644), "block");
    assert_eq!(kind(S_IFSOCK | 0o755), "sock");
    assert_eq!(kind(0o644), "unknown");
}

#[test]
fn line_formats_all_fields() {
    let st = sys::statx::Statx {
        ino: 42,
        mode: sys::stat::S_IFREG | 0o644,
        size: 7,
        dev_major: 8,
        dev_minor: 1,
        mtime_sec: 1_234_567_890,
    };
    let got = line(&st).unwrap();
    assert_eq!(
        got.as_bytes().unwrap(),
        b"kind=file size=7 mode=644 ino=42 dev=8:1 mtime=1234567890\n"
    );
}

#[test]
fn line_masks_file_type_bits_but_keeps_special_bits() {
    // S_IFREG (file type) is masked out of the mode; setuid (0o4000) is a
    // permission bit within 0o7777 and is kept.
    let st = sys::statx::Statx {
        ino: 1,
        mode: sys::stat::S_IFREG | 0o4000 | 0o644,
        size: 0,
        dev_major: 0,
        dev_minor: 0,
        mtime_sec: 0,
    };
    let got = line(&st).unwrap();
    assert!(
        got.as_bytes()
            .unwrap()
            .starts_with(b"kind=file size=0 mode=4644 ")
    );
}

#[test]
fn flags_combine_disjoint_bits() {
    use sys::fcntl::{AT_EMPTY_PATH, AT_SYMLINK_NOFOLLOW};
    assert_eq!(flags(false, false), 0);
    assert_eq!(flags(true, false), AT_SYMLINK_NOFOLLOW);
    assert_eq!(flags(false, true), AT_EMPTY_PATH);
    assert_eq!(flags(true, true), AT_SYMLINK_NOFOLLOW + AT_EMPTY_PATH);
}

#[test]
fn dirfd_default_is_cwd() {
    let state = state_with_memfd();
    let at = dirfd(None, &state).unwrap();
    assert_eq!(at.as_raw(), sys::AtFd::cwd().as_raw());
}

#[test]
fn dirfd_resolves_fd_var() {
    let state = state_with_memfd();
    let var = c"f".into();
    let at = dirfd(Some(&var), &state).unwrap();
    let raw = state.fds.get(&var).unwrap().fd.as_raw();
    assert_eq!(at.as_raw(), raw);
}

#[test]
fn handler_fd_form_succeeds() {
    let state = state_with_memfd();
    with_refs(&["%f"], |refs, origs| {
        assert_eq!(
            handle_statx(c"statx".into(), refs, origs, &state).unwrap(),
            0
        );
    });
}

#[test]
fn handler_unset_fd_var() {
    let state = state_with_memfd();
    with_refs(&["%missing"], |refs, origs| {
        let e = handle_statx(c"statx".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

#[test]
fn handler_unset_dir_var() {
    let state = state_with_memfd();
    with_refs(&["a.txt", "--dir", "%missing"], |refs, origs| {
        let e = handle_statx(c"statx".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

#[test]
fn handler_missing_path_errors() {
    let state = state_with_memfd();
    with_refs(&[], |refs, origs| {
        let e = handle_statx(c"statx".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("path")
        ));
    });
}

#[test]
fn handler_missing_file_is_syscall_error() {
    let state = state_with_memfd();
    with_refs(&["no-such-file-xyz"], |refs, origs| {
        let e = handle_statx(c"statx".into(), refs, origs, &state).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
}
