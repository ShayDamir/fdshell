#![allow(clippy::unwrap_used)]

use super::{Decision, decide};
use sys::stat::{S_IFCHR, S_IFIFO, S_IFREG};

/// The stat gate answers the **mode**, not the errno: a regular file — with its
/// permission bits set, so the `S_IFMT` mask comparison is exercised — is the
/// refusal arm.
#[test]
fn a_regular_mode_is_refused() {
    assert_eq!(decide(Some(S_IFREG | 0o644)), Decision::Refuse);
}

/// Every other file type goes to the plain arm (measured bash rows: the pipe,
/// the character device, the FIFO, the socket, the directory — all open, and
/// the kernel, not bash, decides the errno).
#[test]
fn a_non_regular_mode_goes_to_the_plain_arm() {
    assert_eq!(decide(Some(S_IFIFO | 0o644)), Decision::Plain);
    assert_eq!(decide(Some(S_IFCHR | 0o666)), Decision::Plain);
}

/// The failed-stat arm stays the exclusive create (bash's "the file was not
/// present"): it keeps a create's atomicity and its own errno on failure.
#[test]
fn no_mode_stays_the_exclusive_arm() {
    assert_eq!(decide(None), Decision::Exclusive);
}
