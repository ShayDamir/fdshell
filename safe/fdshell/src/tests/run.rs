#![allow(clippy::unwrap_used)]

use crate::run::run_one;
use crate::task::Task;
use crate::vars::FdVars;
use std::collections::HashMap;
use sys::ShortCStr;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

fn child_test(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            sys::umask::init();
            let saved = sys::umask::get();
            f();
            sys::umask::set(saved);
            std::process::exit(42);
        }
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
            match status {
                WaitStatus::Exited(42) => {}
                other => panic!("unexpected status {}", other.exit_code()),
            }
        }
    }
}

#[test]
fn umask_set_via_run_one() {
    child_test(|| {
        let mut fdvars = FdVars::new();
        let mut tasks = HashMap::new();
        let mut last_status = WaitStatus::Exited(0);
        run_one("umask 0o077", &mut fdvars, &mut tasks, &mut last_status).unwrap();
        assert!(matches!(last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn umask_set_zero_via_run_one() {
    child_test(|| {
        let mut fdvars = FdVars::new();
        let mut tasks = HashMap::new();
        let mut last_status = WaitStatus::Exited(0);
        run_one("umask 0o000", &mut fdvars, &mut tasks, &mut last_status).unwrap();
        assert!(matches!(last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o000);
    });
}

#[test]
fn umask_set_without_o_prefix() {
    child_test(|| {
        let mut fdvars = FdVars::new();
        let mut tasks = HashMap::new();
        let mut last_status = WaitStatus::Exited(0);
        run_one("umask 077", &mut fdvars, &mut tasks, &mut last_status).unwrap();
        assert!(matches!(last_status, WaitStatus::Exited(0)));
        assert_eq!(sys::umask::get(), 0o077);
    });
}

#[test]
fn umask_invalid_returns_err() {
    child_test(|| {
        let mut fdvars = FdVars::new();
        let mut tasks = HashMap::new();
        let mut last_status = WaitStatus::Exited(0);
        let e = run_one("umask abc", &mut fdvars, &mut tasks, &mut last_status).unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn umask_too_many_args_returns_err() {
    child_test(|| {
        let mut fdvars = FdVars::new();
        let mut tasks = HashMap::new();
        let mut last_status = WaitStatus::Exited(0);
        let e = run_one(
            "umask 0o077 extra",
            &mut fdvars,
            &mut tasks,
            &mut last_status,
        )
        .unwrap_err();
        assert_eq!(e, EINVAL);
    });
}

#[test]
fn wait_no_tasks() {
    let mut fdvars = FdVars::new();
    let mut tasks = HashMap::new();
    let mut last_status = WaitStatus::Exited(0);
    run_one("wait", &mut fdvars, &mut tasks, &mut last_status).unwrap();
    assert!(matches!(last_status, WaitStatus::Exited(0)));
}

#[test]
fn wait_nonexistent_name() {
    let mut fdvars = FdVars::new();
    let mut tasks = HashMap::new();
    let mut last_status = WaitStatus::Exited(0);
    let e = run_one(
        "wait &nonexistent",
        &mut fdvars,
        &mut tasks,
        &mut last_status,
    )
    .unwrap_err();
    assert_eq!(e, EINVAL);
}

#[test]
fn wait_one_task() {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => std::process::exit(42),
        Some(pidfd) => {
            let mut fdvars = FdVars::new();
            let mut tasks = HashMap::new();
            tasks.insert(
                ShortCStr::from_static(c"mytask"),
                Task {
                    pidfd,
                    capture_fd: None,
                    child_pid: ret as i32,
                    captures: Vec::new(),
                },
            );
            let mut last_status = WaitStatus::Exited(0);
            run_one("wait &mytask", &mut fdvars, &mut tasks, &mut last_status).unwrap();
            assert!(matches!(last_status, WaitStatus::Exited(42)));
            assert!(tasks.is_empty());
        }
    }
}

#[test]
fn wait_all_tasks() {
    let (ret1, pidfd1_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    let pidfd1 = match pidfd1_opt {
        None => std::process::exit(42),
        Some(pidfd) => pidfd,
    };
    let (ret2, pidfd2_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    let pidfd2 = match pidfd2_opt {
        None => std::process::exit(7),
        Some(pidfd) => pidfd,
    };
    let mut fdvars = FdVars::new();
    let mut tasks = HashMap::new();
    tasks.insert(
        ShortCStr::from_static(c"task1"),
        Task {
            pidfd: pidfd1,
            capture_fd: None,
            child_pid: ret1 as i32,
            captures: Vec::new(),
        },
    );
    tasks.insert(
        ShortCStr::from_static(c"task2"),
        Task {
            pidfd: pidfd2,
            capture_fd: None,
            child_pid: ret2 as i32,
            captures: Vec::new(),
        },
    );
    let mut last_status = WaitStatus::Exited(0);
    run_one("wait", &mut fdvars, &mut tasks, &mut last_status).unwrap();
    let ok = match last_status {
        WaitStatus::Exited(c) => c == 42 || c == 7,
        _ => false,
    };
    assert!(ok);
    assert!(tasks.is_empty());
}

#[test]
fn wait_rejects_capture() {
    let mut fdvars = FdVars::new();
    let mut tasks = HashMap::new();
    let mut last_status = WaitStatus::Exited(0);
    let e = run_one("wait %>%var", &mut fdvars, &mut tasks, &mut last_status).unwrap_err();
    assert_eq!(e, EINVAL);
}
