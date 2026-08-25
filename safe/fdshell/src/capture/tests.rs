#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
use alloc::vec;

use super::commit::{CapturedValue, commit_captured};
use super::{Capture, CapturedFd, do_captures};
use crate::error::capture::CaptureError;
use crate::state::{FdArrayEntry, FdVar, ShellState};
use sys::ShortCStr;
use sys::net::socketpair;
use sys::shellfd::send_fd;

fn short_cstr(s: &'static [u8]) -> ShortCStr {
    ShortCStr::from_vec(s.to_vec()).unwrap()
}

fn pos() -> sys::Position {
    sys::Position::new(1, 1)
}

fn self_pid() -> sys::Pid {
    sys::Pid::from_raw(std::process::id() as i32)
}

fn fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

fn send_one(shell: &sys::LocalFd, tag: &core::ffi::CStr) {
    let (a, b) = socketpair().expect("socketpair");
    a.verify().expect("verify a");
    b.verify().expect("verify b");
    send_fd(shell, &a, tag).expect("send_fd");
    drop(a);
    drop(b);
}

#[test]
fn test_captures_exists() {
    // Create a shell socket to send through and receive from
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    shell_b.verify().expect("verify shell_b");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    // Send an fd so recv_fd succeeds and reaches the Exists check.
    let (test_a, test_b) = socketpair().expect("socketpair");
    test_a.verify().expect("verify test_a");
    test_b.verify().expect("verify test_b");
    send_fd(&shell_sock, &test_a, c"openat2").expect("send_fd");
    drop(test_a);
    drop(test_b);

    let mut state = ShellState::new();
    state.fds.insert(
        short_cstr(b"OUT"),
        FdVar {
            fd: receiver.try_clone().expect("clone"),
            trace: sys::Trace::boundary(sys::Origin::Shell),
        },
    );

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
        cap: None,
        set_at: pos(),
    }];

    let result = do_captures(receiver, self_pid(), captures, &state);
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

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    let (test_a, test_b) = socketpair().expect("socketpair");
    test_a.verify().expect("verify test_a");
    test_b.verify().expect("verify test_b");

    send_fd(&shell_sock, &test_a, c"openat2").expect("send_fd");
    drop(test_a);
    drop(test_b);

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
        cap: None,
        set_at: pos(),
    }];

    let result = do_captures(receiver, self_pid(), captures, &ShellState::new());
    assert!(result.is_ok());
    let captured = result.unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].var.as_bytes().expect("as_bytes"), b"OUT");
    assert!(matches!(captured[0].value, CapturedValue::Fd(_)));
}

#[test]
fn test_captures_skips_last_arg_tag() {
    // A `$_`-tagged message (last-arg report) must be skipped, not captured.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    send_one(&shell_sock, c"$_");
    send_one(&shell_sock, c"openat2");

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
        cap: None,
        set_at: pos(),
    }];

    let result = do_captures(receiver, self_pid(), captures, &ShellState::new());
    let captured = result.expect("captures should succeed past the $_ tag");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].var.as_bytes().expect("as_bytes"), b"OUT");
}

#[test]
fn test_captures_incomplete_zero() {
    // Socket closes before any fd is sent — expect Incomplete { expected: 1, received: 0 }
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    shell_b.verify().expect("verify shell_b");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    drop(shell_a);

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
        cap: None,
        set_at: pos(),
    }];

    let result = do_captures(receiver, self_pid(), captures, &ShellState::new());
    match result {
        Err(e)
            if matches!(
                *e.current_context(),
                CaptureError::Incomplete {
                    expected: 1,
                    received: 0
                }
            ) => {}
        _other => panic!("expected Incomplete {{ expected: 1, received: 0 }}"),
    }
}

#[test]
fn test_captures_incomplete_partial() {
    // 2 captures, only 1 fd sent — expect Incomplete { expected: 2, received: 1 }
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    shell_b.verify().expect("verify shell_b");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    // Send first fd with matching tag
    send_one(&shell_sock, c"tag1");

    // Second sender — export then drop to close the socket (no second fd)
    let (test2_a, test2_b) = socketpair().expect("socketpair");
    test2_a.verify().expect("verify test2_a");
    test2_b.verify().expect("verify test2_b");
    test2_a.export().expect("export test2_a");
    drop(test2_a);
    drop(test2_b);

    let captures = vec![
        Capture {
            var: short_cstr(b"OUT1"),
            tag: Some(short_cstr(b"tag1")),
            force: false,
            cap: None,
            set_at: pos(),
        },
        Capture {
            var: short_cstr(b"OUT2"),
            tag: Some(short_cstr(b"tag2")),
            force: false,
            cap: None,
            set_at: pos(),
        },
    ];

    let result = do_captures(receiver, self_pid(), captures, &ShellState::new());
    match result {
        Err(e)
            if matches!(
                *e.current_context(),
                CaptureError::Incomplete {
                    expected: 2,
                    received: 1
                }
            ) => {}
        _other => panic!("expected Incomplete {{ expected: 2, received: 1 }}"),
    }
}

#[test]
fn test_bounded_capture_collects_up_to_cap() {
    // cap 2, 3 fds sent: 2 collected, the third closed.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    shell_b.verify().expect("verify shell_b");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    for _ in 0..3 {
        send_one(&shell_sock, c"conn");
    }
    drop(shell_sock);

    let before = fd_count();
    let captures = vec![Capture {
        var: short_cstr(b"arr"),
        tag: Some(short_cstr(b"conn")),
        force: false,
        cap: Some(2),
        set_at: pos(),
    }];
    let captured =
        do_captures(receiver, self_pid(), captures, &ShellState::new()).expect("bounded capture");
    assert_eq!(captured.len(), 1);
    match &captured[0].value {
        CapturedValue::Array(entries) => assert_eq!(entries.len(), 2),
        _other => panic!("expected Array value"),
    }
    // Two fds are open in `captured`, the receiver socket is closed: net +1.
    assert_eq!(fd_count(), before + 1);
}

#[test]
fn test_bounded_capture_fewer_than_cap_is_ok() {
    // cap 3, 1 fd sent: succeeds with one entry.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    send_one(&shell_sock, c"conn");
    drop(shell_sock);

    let captures = vec![Capture {
        var: short_cstr(b"arr"),
        tag: Some(short_cstr(b"conn")),
        force: false,
        cap: Some(3),
        set_at: pos(),
    }];
    let captured = do_captures(receiver, self_pid(), captures, &ShellState::new())
        .expect("bounded capture under cap");
    match &captured[0].value {
        CapturedValue::Array(entries) => assert_eq!(entries.len(), 1),
        _other => panic!("expected Array value"),
    }
}

#[test]
fn test_bounded_capture_respects_existing_array_length() {
    // existing array of 1, cap 3: at most 2 more collected.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    for _ in 0..3 {
        send_one(&shell_sock, c"conn");
    }
    drop(shell_sock);

    let mut state = ShellState::new();
    let (seed_a, seed_b) = socketpair().expect("socketpair");
    seed_a.verify().expect("verify seed_a");
    seed_b.verify().expect("verify seed_b");
    state.arrays.insert(
        short_cstr(b"arr"),
        vec![FdArrayEntry {
            fd: seed_b,
            source: short_cstr(b"seed"),
            trace: sys::Trace::boundary(sys::Origin::Shell),
        }],
    );
    drop(seed_a);

    let captures = vec![Capture {
        var: short_cstr(b"arr"),
        tag: Some(short_cstr(b"conn")),
        force: false,
        cap: Some(3),
        set_at: pos(),
    }];
    let captured = do_captures(receiver, self_pid(), captures, &state)
        .expect("bounded capture with existing array");
    match &captured[0].value {
        CapturedValue::Array(entries) => assert_eq!(entries.len(), 2),
        _other => panic!("expected Array value"),
    }
}

#[test]
fn test_bounded_capture_full_array_closes_all() {
    // existing array already at cap: every received fd is closed.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    for _ in 0..2 {
        send_one(&shell_sock, c"conn");
    }
    drop(shell_sock);

    let mut state = ShellState::new();
    let (s1a, s1b) = socketpair().expect("socketpair");
    let (s2a, s2b) = socketpair().expect("socketpair");
    state.arrays.insert(
        short_cstr(b"arr"),
        vec![
            FdArrayEntry {
                fd: s1b,
                source: short_cstr(b"s1"),
                trace: sys::Trace::boundary(sys::Origin::Shell),
            },
            FdArrayEntry {
                fd: s2b,
                source: short_cstr(b"s2"),
                trace: sys::Trace::boundary(sys::Origin::Shell),
            },
        ],
    );
    drop(s1a);
    drop(s2a);

    let before = fd_count();
    let captures = vec![Capture {
        var: short_cstr(b"arr"),
        tag: Some(short_cstr(b"conn")),
        force: false,
        cap: Some(2),
        set_at: pos(),
    }];
    let captured =
        do_captures(receiver, self_pid(), captures, &state).expect("full bounded capture");
    match &captured[0].value {
        CapturedValue::Array(entries) => assert!(entries.is_empty()),
        _other => panic!("expected Array value"),
    }
    // Receiver closed; the two buffered fds were never taken into the process: net -1.
    assert_eq!(fd_count(), before - 1);
}

#[test]
fn test_bounded_capture_scalar_conflict_is_exists() {
    // Target is a scalar fd var and force is off — Exists.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    shell_b.verify().expect("verify shell_b");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    drop(shell_a);

    let mut state = ShellState::new();
    state.fds.insert(
        short_cstr(b"arr"),
        FdVar {
            fd: receiver.try_clone().expect("clone"),
            trace: sys::Trace::boundary(sys::Origin::Shell),
        },
    );

    let captures = vec![Capture {
        var: short_cstr(b"arr"),
        tag: None,
        force: false,
        cap: Some(2),
        set_at: pos(),
    }];
    let result = do_captures(receiver, self_pid(), captures, &state);
    match result {
        Err(e) if matches!(*e.current_context(), CaptureError::Exists) => {}
        _other => panic!("expected Exists"),
    }
}

#[test]
fn test_bounded_capture_untagged_accepts_any_tag() {
    // `%>%arr[N]` (no tag) captures the first fd of any tag.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    send_one(&shell_sock, c"whatever");
    drop(shell_sock);

    let captures = vec![Capture {
        var: short_cstr(b"arr"),
        tag: None,
        force: false,
        cap: Some(1),
        set_at: pos(),
    }];
    let captured = do_captures(receiver, self_pid(), captures, &ShellState::new())
        .expect("untagged bounded capture");
    let entries = match &captured[0].value {
        CapturedValue::Array(entries) => entries,
        _other => panic!("expected Array value"),
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].source.as_bytes().expect("as_bytes"), b"whatever");
}

#[test]
fn test_commit_captured_array() {
    // Array values commit into `arrays`, displacing a scalar of the same name.
    let (a1, b1) = socketpair().expect("socketpair");
    a1.verify().expect("verify a1");
    b1.verify().expect("verify b1");
    let (a2, b2) = socketpair().expect("socketpair");
    a2.verify().expect("verify a2");
    b2.verify().expect("verify b2");
    let (sa, sb) = socketpair().expect("socketpair");
    sa.verify().expect("verify sa");
    sb.verify().expect("verify sb");

    let mut state = ShellState::new();
    state.fds.insert(
        short_cstr(b"arr"),
        FdVar {
            fd: sa,
            trace: sys::Trace::boundary(sys::Origin::Shell),
        },
    );

    let captured = vec![CapturedFd {
        var: short_cstr(b"arr"),
        value: CapturedValue::Array(vec![
            FdArrayEntry {
                fd: b1,
                source: short_cstr(b"t1"),
                trace: sys::Trace::boundary(sys::Origin::Captured(short_cstr(b"t1"))),
            },
            FdArrayEntry {
                fd: b2,
                source: short_cstr(b"t2"),
                trace: sys::Trace::boundary(sys::Origin::Captured(short_cstr(b"t2"))),
            },
        ]),
    }];
    commit_captured(&mut state, captured);

    assert!(!state.fds.contains_key(&short_cstr(b"arr")));
    let arr = state
        .arrays
        .get(&short_cstr(b"arr"))
        .expect("array committed");
    assert_eq!(arr.len(), 2);
    drop(a1);
    drop(a2);
    drop(sb);
}

#[test]
fn test_tag_mismatch_is_not_captured() {
    // Slot expects tag "openat2"; an fd tagged "mkdirat" must not land in it.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    send_one(&shell_sock, c"mkdirat");
    drop(shell_sock);

    let captures = vec![Capture {
        var: short_cstr(b"OUT"),
        tag: Some(short_cstr(b"openat2")),
        force: false,
        cap: Some(1),
        set_at: pos(),
    }];
    let captured = do_captures(receiver, self_pid(), captures, &ShellState::new())
        .expect("tag mismatch leaves slot empty but satisfied");
    match &captured[0].value {
        CapturedValue::Array(entries) => assert!(entries.is_empty()),
        _other => panic!("expected Array value"),
    }
}

#[test]
fn test_multi_slot_full_slot_does_not_steal_matching_tag() {
    // Slot A (tag "a", cap 1) fills; a second "a" fd must go to the
    // untagged slot B, not be absorbed by the full slot A.
    let (shell_a, shell_b) = socketpair().expect("socketpair");
    shell_a.verify().expect("verify shell_a");
    shell_b.verify().expect("verify shell_b");
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().expect("export shell_a");
    let shell_sock = shell_a.try_clone().expect("clone shell");
    drop(shell_a);

    send_one(&shell_sock, c"a");
    send_one(&shell_sock, c"a");
    drop(shell_sock);

    let captures = vec![
        Capture {
            var: short_cstr(b"arr_a"),
            tag: Some(short_cstr(b"a")),
            force: false,
            cap: Some(1),
            set_at: pos(),
        },
        Capture {
            var: short_cstr(b"arr_b"),
            tag: None,
            force: false,
            cap: Some(1),
            set_at: pos(),
        },
    ];
    let captured = do_captures(receiver, self_pid(), captures, &ShellState::new())
        .expect("multi-slot capture");
    assert_eq!(captured.len(), 2);
    let count = |c: &CapturedFd| match &c.value {
        CapturedValue::Array(entries) => entries.len(),
        _other => panic!("expected Array value"),
    };
    assert_eq!(count(&captured[0]), 1);
    assert_eq!(count(&captured[1]), 1);
}
