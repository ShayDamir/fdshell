#![cfg_attr(test, allow(clippy::unwrap_used))]

use sys::signalfd::{self, SFD_CLOEXEC, SIGUSR1};

#[test]
fn create_sets_cloexec() {
    let fd = signalfd::signalfd(&[SIGUSR1], 0).unwrap();
    // A stub that drops `| SFD_CLOEXEC` (or uses fd 1) would fail this.
    fd.verify().unwrap();
}

#[test]
fn create_keeps_cloexec_when_requested() {
    let fd = signalfd::signalfd(&[SIGUSR1], SFD_CLOEXEC).unwrap();
    // A `^` instead of `|` would clear CLOEXEC when it is already set.
    fd.verify().unwrap();
}

#[test]
fn create_blocks_the_signal_in_mask() {
    // The signalfd must block SIGUSR1 so it is delivered via the fd, not the
    // default disposition. A stub that skips `sigprocmask` would leave the
    // signal unblocked. (Checked via the mask, not a real signal, so this is
    // safe in a multi-threaded test process.)
    let _fd = signalfd::signalfd(&[SIGUSR1], 0).unwrap();
    assert!(
        signalfd::signal_blocked(SIGUSR1),
        "SIGUSR1 must be blocked after signalfd"
    );
    // A signal not in the mask must report unblocked (kills a stub that
    // always returns `true`).
    assert!(
        !signalfd::signal_blocked(signalfd::SIGUSR2),
        "SIGUSR2 must stay unblocked"
    );
}

#[test]
fn create_rejects_bad_signal() {
    // Signal 0 is not a valid signal number for `sigaddset`.
    match signalfd::signalfd(&[0], 0) {
        Err(err) => assert!(matches!(err, sys::SyscallError::EINVAL(_)), "got {err}"),
        Ok(_) => panic!("expected an error for signal 0"),
    }
}
