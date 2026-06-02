#![cfg_attr(test, allow(clippy::unwrap_used))]
use super::*;
use sys::siginfo::WaitStatus;

fn child_test(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            f();
            std::process::exit(42);
        }
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
            match status {
                WaitStatus::Exited(42) => {}
                other => panic!("unexpected {other:?}"),
            }
        }
    }
}

// TODO: this hangs sometimes (interaction with parallel test runner?).
// The child forks and immediately tries fchdir(/tmp). Something blocks
// or waits on an fd that isn't ready. Needs investigation.
#[test]
#[ignore]
fn cd_to_tmp() {
    child_test(|| {
        let mut v = FdVars::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut v).unwrap();
        let cwd = v.resolve(&c"CWD".into()).unwrap();
        cwd.verify().unwrap();
    });
}

#[test]
fn cd_to_home() {
    let home_exists = std::env::var_os("HOME").is_some_and(|p| std::path::Path::new(&p).exists());
    if home_exists {
        child_test(|| {
            let mut v = FdVars::new();
            cd(&[], &mut v).unwrap();
            let cwd = v.resolve(&c"CWD".into()).unwrap();
            cwd.verify().unwrap();
        });
    } else {
        child_test(|| {
            let mut v = FdVars::new();
            let e = cd(&[], &mut v).unwrap_err();
            assert_eq!(e, sys::errno::ENOENT);
        });
    }
}

#[test]
fn cd_to_self() {
    child_test(|| {
        let mut v = FdVars::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut v).unwrap();
        let cwd_fd = v.resolve(&c"CWD".into()).unwrap().try_clone().unwrap();
        v.insert(c"CWD".into(), cwd_fd);
        cd(&[c"%CWD".into()], &mut v).unwrap();
        let cwd = v.resolve(&c"CWD".into()).unwrap();
        cwd.verify().unwrap();
    });
}

#[test]
fn cd_missing_path() {
    child_test(|| {
        let mut v = FdVars::new();
        let bad = c"/nonexistent-cd-test-xxxxxxxx".into();
        let e = cd(&[bad], &mut v).unwrap_err();
        assert_eq!(e, sys::errno::ENOENT);
    });
}

#[test]
fn cd_missing_var() {
    child_test(|| {
        let mut v = FdVars::new();
        let bad = c"%NONEXISTENT".into();
        let e = cd(&[bad], &mut v).unwrap_err();
        assert_eq!(e, sys::errno::ENOENT);
    });
}

#[test]
fn cd_dash_switches_to_oldpwd() {
    child_test(|| {
        let mut v = FdVars::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut v).unwrap();
        let root = c"/".into();
        cd(&[root], &mut v).unwrap();
        let dash = c"-".into();
        cd(&[dash], &mut v).unwrap();
        let cwd = v.resolve(&c"CWD".into()).unwrap();
        cwd.verify().unwrap();
        let old = v.resolve(&c"OLDCWD".into()).unwrap();
        old.verify().unwrap();
    });
}

#[test]
fn cd_move_cwd_to_oldcwd() {
    child_test(|| {
        let mut v = FdVars::new();
        let tmp = c"/tmp".into();
        cd(&[tmp], &mut v).unwrap();
        assert!(v.resolve(&c"CWD".into()).is_some());
        assert!(v.resolve(&c"OLDCWD".into()).is_none());
        let root = c"/".into();
        cd(&[root], &mut v).unwrap();
        assert!(v.resolve(&c"CWD".into()).is_some());
        assert!(v.resolve(&c"OLDCWD".into()).is_some());
    });
}
