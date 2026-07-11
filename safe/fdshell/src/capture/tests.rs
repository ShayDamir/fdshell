#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use super::{Capture, do_captures};
use crate::error::capture::CaptureError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::net::socketpair;
use sys::shellfd::{send_fd, set_capture_active};

fn short_cstr(s: &'static [u8]) -> ShortCStr {
    ShortCStr::from_vec(s.to_vec()).unwrap()
}

#[test]
fn test_captures_exists() {
    let (a, b) = socketpair().expect("socketpair");
    a.verify().expect("verify a");
    b.verify().expect("verify b");

    // Send an fd so recv_fd succeeds and reaches the Exists check.
    let (test_a, test_b) = socketpair().expect("socketpair");
    test_a.verify().expect("verify test_a");
    test_b.verify().expect("verify test_b");
    set_capture_active(true);
    send_fd(&test_a, c"openat2").expect("send_fd");
    test_a.try_close().expect("close test_a");
    test_b.try_close().expect("close test_b");

    let mut state = ShellState::new();
    state
        .fds
        .insert(short_cstr(b"OUT"), a.try_clone().expect("clone"));

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
    }];

    let result = do_captures(b, std::process::id() as i32, captures, &state);
    match result {
        Err(e) if matches!(*e.current_context(), CaptureError::Exists) => {}
        _other => panic!("expected Exists"),
    }
}

#[test]
fn test_captures_success() {
    let (a, b) = socketpair().expect("socketpair");
    a.verify().expect("verify a");
    b.verify().expect("verify b");

    let (test_a, test_b) = socketpair().expect("socketpair");
    test_a.verify().expect("verify test_a");
    test_b.verify().expect("verify test_b");

    set_capture_active(true);
    send_fd(&test_a, c"openat2").expect("send_fd");
    test_a.try_close().expect("close test_a");
    test_b.try_close().expect("close test_b");

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
    }];

    let result = do_captures(b, std::process::id() as i32, captures, &ShellState::new());
    assert!(result.is_ok());
    let captured = result.unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].0.as_bytes().expect("as_bytes"), b"OUT");
}
