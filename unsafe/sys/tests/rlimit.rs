#![allow(clippy::unwrap_used)]

use sys::SyscallError;
use sys::rlimit::{self, RLimit};

#[test]
fn get_nofile_returns_sane_pair() {
    let lim = rlimit::get(rlimit::NOFILE).unwrap();
    assert!(lim.soft > 0);
    assert!(lim.hard >= lim.soft);
}

#[test]
fn get_rss_is_unlimited_by_default() {
    let lim = rlimit::get(rlimit::RSS).unwrap();
    assert_eq!(lim.soft, rlimit::UNLIMITED);
    assert_eq!(lim.hard, rlimit::UNLIMITED);
}

#[test]
fn set_nofile_soft_lower_and_restore() {
    let before = rlimit::get(rlimit::NOFILE).unwrap();
    rlimit::set(
        rlimit::NOFILE,
        RLimit {
            soft: 1,
            hard: before.hard,
        },
    )
    .unwrap();
    // A stub returning Ok without acting would keep the old soft limit.
    let after = rlimit::get(rlimit::NOFILE).unwrap();
    assert_eq!(after.soft, 1);
    assert_eq!(after.hard, before.hard);
    rlimit::set(rlimit::NOFILE, before).unwrap();
    assert_eq!(rlimit::get(rlimit::NOFILE).unwrap(), before);
}

#[test]
fn set_nofile_hard_lower() {
    let before = rlimit::get(rlimit::NOFILE).unwrap();
    let hard = before.hard.max(2) / 2;
    rlimit::set(rlimit::NOFILE, RLimit { soft: 1, hard }).unwrap();
    // A stub returning Ok without acting would keep the old hard limit.
    let after = rlimit::get(rlimit::NOFILE).unwrap();
    assert_eq!(after.hard, hard);
    // The original hard limit is not restored: raising a hard limit requires
    // privilege. nextest runs each test in its own process, so the lowered
    // limit dies with it.
}

#[test]
fn unknown_resource_errors() {
    assert_eq!(
        rlimit::get(99).unwrap_err(),
        SyscallError::EINVAL("unknown")
    );
    assert_eq!(
        rlimit::set(99, RLimit { soft: 1, hard: 1 }).unwrap_err(),
        SyscallError::EINVAL("unknown")
    );
}
