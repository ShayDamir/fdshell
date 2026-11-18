#![allow(clippy::unwrap_used)]

//! Pins for the `getpwuid(3)` wrapper (task #614). Only values true in both
//! test environments are pinned (plan §3): the `root` and `nobody` entries
//! every password file has, and a uid no backend answers.

use sys::getpwuid::getpwuid;

#[test]
fn uid_zero_is_root() {
    let want: &[u8] = b"root";
    assert_eq!(getpwuid(0).as_deref(), Some(want));
}

#[test]
fn uid_65534_is_nobody() {
    let want: &[u8] = b"nobody";
    assert_eq!(getpwuid(65534).as_deref(), Some(want));
}

/// No backend answers this uid on host or in the sandbox (measured: plan §3),
/// so the `None` arm is deterministic in both environments.
#[test]
fn unassigned_uid_is_none() {
    assert_eq!(getpwuid(2_147_483_647), None);
}
