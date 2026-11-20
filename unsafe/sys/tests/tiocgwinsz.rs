#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Pins for the `TIOCGWINSZ` wrapper, driven through `sys::pty::openpty()` in
//! the wrapper's own test binary
//! (LESSONS: "`cargo mutants` runs only the mutated package's own tests unless `--test-workspace true`").
//! The four fields come back from a size the test sets, a fresh pty's size is
//! `0 x 0`, and a pipe fd answers `ENOTTY`
//! (LESSONS: "A pty's termios baseline is host state, so pins are deltas against a snapshot").

use sys::pty::openpty;
use sys::tiocgwinsz::{self, Winsize};

/// A size the test sets with `TIOCSWINSZ` comes back field for field — the
/// wrapper cannot answer `Default::default()` and still pass.
#[test]
fn a_set_size_comes_back_field_for_field() {
    let (_master, slave) = openpty().unwrap();
    let fd = slave.as_raw();
    let want = Winsize {
        rows: 40,
        cols: 132,
        xpixel: 7,
        ypixel: 3,
    };
    let raw = libc::winsize {
        ws_row: want.rows,
        ws_col: want.cols,
        ws_xpixel: want.xpixel,
        ws_ypixel: want.ypixel,
    };
    // SAFETY: `fd` is the open slave of this test's own pty and `raw` is a
    // valid `libc::winsize` the ioctl reads.
    let rc = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &raw) };
    assert_eq!(rc, 0, "TIOCSWINSZ on a pty slave must succeed");
    assert_eq!(tiocgwinsz::get(fd).unwrap(), want);
}

/// A freshly allocated pty reports an all-zero size (measured), so the plain
/// struct has a pin and no test may assume a non-zero size.
#[test]
fn a_fresh_pty_reports_a_zero_size() {
    let (_master, slave) = openpty().unwrap();
    assert_eq!(tiocgwinsz::get(slave.as_raw()).unwrap(), Winsize::default());
}

/// A pipe is not a terminal: the ioctl answers `ENOTTY` (measured; not a named
/// `SyscallError` variant), so the wrapper reports it through `cvt`.
#[test]
fn a_pipe_fd_answers_enotty() {
    let (reader, _writer) = sys::pipe::pipe2(0).unwrap();
    assert_eq!(
        tiocgwinsz::get(reader.as_raw()).unwrap_err().errno(),
        libc::ENOTTY
    );
}
