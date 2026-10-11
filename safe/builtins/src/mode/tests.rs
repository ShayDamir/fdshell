#![allow(clippy::unwrap_used)]

use super::*;
use crate::error::BuiltinError;

/// The unit tests run with the process umask cache uninitialized
/// (`sys::umask::init` is only called by the fdshell binary), so the guard
/// sees `effective == requested` and every case below is a pure bit-mask
/// boundary. The mask itself is exercised end to end by `tests/mkdirat.rs`
/// and `tests/openat2_mode.rs`, which set the umask from the shell.
fn passes(requested: u32, needed: u32) -> bool {
    require_reopenable(requested, needed).is_ok()
}

#[test]
fn dir_defaults_and_boundaries() {
    assert!(passes(DIR_MODE, DIR_REOPEN), "0777 default must be usable");
    assert!(passes(0o777, DIR_REOPEN));
    assert!(passes(0o755, DIR_REOPEN));
    assert!(passes(0o500, DIR_REOPEN), "owner r+x is exactly enough");
}

#[test]
fn dir_reopen_denied_below_the_boundary() {
    assert!(!passes(0o400, DIR_REOPEN), "owner x missing");
    assert!(!passes(0o100, DIR_REOPEN), "owner w only");
    assert!(
        !passes(0o077, DIR_REOPEN),
        "group/other bits are not the owner"
    );
    assert!(!passes(0o000, DIR_REOPEN), "mode 000 is the reported trap");
}

#[test]
fn fifo_defaults_and_boundaries() {
    assert!(
        passes(CREATE_MODE, FIFO_REOPEN),
        "0666 default must be usable"
    );
    assert!(passes(0o666, FIFO_REOPEN));
    assert!(passes(0o644, FIFO_REOPEN));
    assert!(passes(0o600, FIFO_REOPEN), "owner r+w is exactly enough");
}

#[test]
fn fifo_reopen_denied_below_the_boundary() {
    assert!(!passes(0o500, FIFO_REOPEN), "owner w missing");
    assert!(!passes(0o200, FIFO_REOPEN), "owner w only");
    assert!(!passes(0o000, FIFO_REOPEN));
}

#[test]
fn special_bits_do_not_help_the_owner_bits() {
    assert!(passes(0o1755, DIR_REOPEN), "sticky bit keeps 755");
    assert!(!passes(0o1400, DIR_REOPEN), "setuid does not add owner x");
    assert!(!passes(0o7400, FIFO_REOPEN), "setgid does not add owner w");
}

#[test]
fn denial_is_reported_as_the_mode_variant() {
    let e = require_reopenable(0o000, DIR_REOPEN).unwrap_err();
    assert!(matches!(
        e.current_context(),
        BuiltinError::ModeReopenDenied
    ));
}
