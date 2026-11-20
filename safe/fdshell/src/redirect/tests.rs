#![allow(clippy::unwrap_used)]
use super::*;
use alloc::format;

/// What fd `n` points at, through `/proc/self/fd/N` (a symlink to the open file).
fn link(n: i32) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/self/fd/{n}")).ok()
}

/// A `CLOEXEC` copy of fd 0, so the test owns a fd number it can free.
fn own_fd() -> LocalFd {
    sys::dup::dup_cloexec(0).unwrap()
}

#[test]
fn close_of_a_never_opened_fd_is_silent() {
    // The def asks for the state the fd is already in. `close(2)` answers `EBADF`,
    // bash ignores that errno and continues, so the apply path reports success
    // (§4.12) — the agreement with the restore arm's `let _ = close(*target)`.
    let n = own_fd().as_raw();
    // The `LocalFd` is dropped here, which closes the number (§5.9).
    assert!(sys::dup::dup_cloexec(n).is_err(), "the target is not open");
    Redirect::Close { export_to: n }.export().unwrap();
    assert!(
        sys::dup::dup_cloexec(n).is_err(),
        "nothing was open, so nothing is reported"
    );
}

#[test]
fn close_of_an_open_fd_actually_closes_it() {
    // A live fd keeps the close real: `Ok(())` alone is not the assertion, the fd
    // must be gone afterwards. This is the unit-level kill of the `export()` arm —
    // the script-level twin is `tests/redirect.rs::reopen_then_close_stdin`.
    let opened = own_fd();
    let n = opened.as_raw();
    assert_eq!(link(n), link(0), "the target is a live fd");
    Redirect::Close { export_to: n }.export().unwrap();
    assert!(sys::dup::dup_cloexec(n).is_err(), "the close landed");
    // The number is free again, so the owner's `Drop` close is a no-op.
    drop(opened);
}
