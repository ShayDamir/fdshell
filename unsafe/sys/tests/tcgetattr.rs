#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Pins for the `tcgetattr`/`tcsetattr` wrappers, driven through
//! `sys::pty::openpty()` in each wrapper's own test binary
//! (LESSONS: "`cargo mutants` runs only the mutated package's own tests unless `--test-workspace true`").
//! Every pin is a delta against a saved snapshot, never an absolute flag number
//! (LESSONS: "A pty's termios baseline is host state, so pins are deltas against a snapshot").

use sys::pty::openpty;
use sys::tcgetattr::{TCSADRAIN, TCSANOW, Termios};

/// Set control-character slot `index` of `t` to `value`; tests keep indexing
/// out too.
fn set_cc(t: &mut Termios, index: usize, value: u8) {
    *t.cc
        .get_mut(index)
        .expect("the control-character table is NCCS long") = value;
}

/// get → change one field → set → get. The change must come back and every
/// other field must still be the one already there; a `save` that answers
/// `Default::default()` or an `apply` that only returns `Ok(())` breaks the
/// observable-change leg. The write drains first: output may be queued.
#[test]
fn save_apply_round_trip_is_observable() {
    let (_master, slave) = openpty().unwrap();
    let fd = slave.as_raw();
    let baseline = Termios::save(fd).unwrap();
    let mut want = baseline;
    want.iflag &= !libc::IXON;
    assert_ne!(want, baseline, "the measured pty baseline has IXON set");
    want.apply(fd, TCSADRAIN).unwrap();
    assert_eq!(Termios::save(fd).unwrap(), want);
    baseline.apply(fd, TCSANOW).unwrap();
    assert_eq!(Termios::save(fd).unwrap(), baseline);
}

/// `enter_raw` clears exactly `ICANON | ECHO | ISIG | IXON`, writes `VMIN = 1`
/// and `VTIME = 0`, and touches nothing else. Both control characters start at
/// sentinels so their writes are observable, and the guard keeps the test from
/// going vacuous on a host whose baseline lacks those bits.
#[test]
fn enter_raw_applies_exactly_the_measured_set() {
    let (_master, slave) = openpty().unwrap();
    let fd = slave.as_raw();
    let baseline = Termios::save(fd).unwrap();
    let mut want = baseline;
    want.iflag &= !libc::IXON;
    want.lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
    set_cc(&mut want, libc::VMIN, 1);
    set_cc(&mut want, libc::VTIME, 0);
    assert_ne!(
        want, baseline,
        "the measured pty baseline has the four bits set"
    );
    let mut probe = baseline;
    set_cc(&mut probe, libc::VMIN, 5);
    set_cc(&mut probe, libc::VTIME, 7);
    probe.enter_raw();
    assert_eq!(probe, want, "the flag and control-character set is exact");
    probe.apply(fd, TCSANOW).unwrap();
    assert_eq!(
        Termios::save(fd).unwrap(),
        want,
        "the set is observable on the terminal"
    );
}

/// The exit path: enter raw, apply, then restore the value saved before any
/// change — the terminal answers the baseline again.
#[test]
fn restore_round_trip_returns_the_saved_mode() {
    let (_master, slave) = openpty().unwrap();
    let fd = slave.as_raw();
    let baseline = Termios::save(fd).unwrap();
    let mut raw = baseline;
    raw.enter_raw();
    assert_ne!(
        raw, baseline,
        "the measured pty baseline has the four bits set"
    );
    raw.apply(fd, TCSANOW).unwrap();
    assert_ne!(
        Termios::save(fd).unwrap(),
        baseline,
        "raw mode must be observable on the terminal"
    );
    baseline.apply(fd, TCSANOW).unwrap();
    assert_eq!(
        Termios::save(fd).unwrap(),
        baseline,
        "the saved mode must come back"
    );
}
