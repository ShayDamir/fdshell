#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use super::{Capture, do_captures};
use crate::error::capture::CaptureError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::net::socketpair;
use sys::shellfd::send_fd;

fn short_cstr(s: &'static [u8]) -> ShortCStr {
    ShortCStr::from_vec(s.to_vec()).unwrap()
}

#[test]
fn test_captures_exists() {
    // Create a shell socket to send through and receive from
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    shell_b.verify().expect("verify shell_b");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export_to(3).expect("export shell_a to fd 3");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    shell_a.try_close().expect("close shell_a");

    // Send an fd so recv_fd succeeds and reaches the Exists check.
    let (test_a, test_b) = socketpair().expect("socketpair");
    test_a.verify().expect("verify test_a");
    test_b.verify().expect("verify test_b");
    send_fd(&shell_sock, &test_a, c"openat2").expect("send_fd");
    test_a.try_close().expect("close test_a");
    test_b.try_close().expect("close test_b");

    let mut state = ShellState::new();
    state
        .fds
        .insert(short_cstr(b"OUT"), receiver.try_clone().expect("clone"));

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
    }];

    let result = do_captures(receiver, std::process::id() as i32, captures, &state);
    match result {
        Err(e) if matches!(*e.current_context(), CaptureError::Exists) => {}
        _other => panic!("expected Exists"),
    }
}

#[test]
fn test_captures_success() {
    // Create a shell socket to send through and receive from
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    shell_b.verify().expect("verify shell_b");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export_to(3).expect("export shell_a to fd 3");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    shell_a.try_close().expect("close shell_a");

    let (test_a, test_b) = socketpair().expect("socketpair");
    test_a.verify().expect("verify test_a");
    test_b.verify().expect("verify test_b");

    send_fd(&shell_sock, &test_a, c"openat2").expect("send_fd");
    test_a.try_close().expect("close test_a");
    test_b.try_close().expect("close test_b");

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
    }];

    let result = do_captures(
        receiver,
        std::process::id() as i32,
        captures,
        &ShellState::new(),
    );
    assert!(result.is_ok());
    let captured = result.unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].0.as_bytes().expect("as_bytes"), b"OUT");
}
