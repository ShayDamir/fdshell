#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use sys::shellfd::TAG_MAX;
use sys::timerfd::TFD_NONBLOCK;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| match builtins::timerfd::parse::timerfd_parse(a) {
        Err(e) => {
            let ctx = e.current_context();
            match (ctx, expected) {
                (BuiltinError::Help, BuiltinError::Help) => {}
                (BuiltinError::MissingArgument(_), BuiltinError::MissingArgument(_)) => {}
                (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                _ => panic!("unexpected error: {ctx}"),
            }
        }
        _ => panic!("expected Err"),
    });
}

fn assert_ok<F: Fn(&builtins::timerfd::parse::TimerfdConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::timerfd::parse::timerfd_parse(a) {
        Ok(cfg) => f(&cfg),
        _ => panic!("expected Ok"),
    });
}

#[test]
fn help_long() {
    assert_err(&["--help"], BuiltinError::Help);
}

#[test]
fn help_short() {
    assert_err(&["-h"], BuiltinError::Help);
}

#[test]
fn missing_seconds() {
    assert_err(&[], BuiltinError::MissingArgument("seconds"));
}

#[test]
fn basic_one_shot() {
    assert_ok(&["5"], |cfg| {
        assert_eq!(cfg.value_sec, 5);
        assert_eq!(cfg.value_nsec, 0);
        assert_eq!(cfg.interval_sec, 0);
        assert_eq!(cfg.interval_nsec, 0);
        assert_eq!(cfg.flags, 0);
    });
}

#[test]
fn with_nanos() {
    assert_ok(&["5", "500000000"], |cfg| {
        assert_eq!(cfg.value_sec, 5);
        assert_eq!(cfg.value_nsec, 500_000_000);
    });
}

#[test]
fn periodic_repeats_value() {
    assert_ok(&["5", "500000000", "--periodic"], |cfg| {
        assert_eq!(cfg.interval_sec, 5);
        assert_eq!(cfg.interval_nsec, 500_000_000);
    });
}

#[test]
fn flags_nonblock() {
    assert_ok(&["5", "--flags", "TFD_NONBLOCK"], |cfg| {
        assert_eq!(cfg.flags, TFD_NONBLOCK);
    });
}

#[test]
fn flags_hex() {
    assert_ok(&["5", "--flags", "0x800"], |cfg| {
        assert_eq!(cfg.flags, TFD_NONBLOCK);
    });
}

#[test]
fn bad_seconds() {
    assert_err(&["abc"], BuiltinError::InvalidArgument("seconds"));
}

#[test]
fn negative_seconds() {
    assert_err(&["-1"], BuiltinError::InvalidArgument("flag"));
}

#[test]
fn unknown_flag_is_flag_error() {
    with_args(&["-x"], |a| {
        match builtins::timerfd::parse::timerfd_parse(a) {
            Err(e) => {
                let ctx = e.current_context();
                assert!(
                    matches!(ctx, BuiltinError::InvalidArgument(msg) if *msg == "flag"),
                    "expected flag error, got {ctx}"
                );
            }
            _ => panic!("expected Err"),
        }
    });
}

#[test]
fn zero_seconds_is_valid() {
    assert_ok(&["0"], |cfg| {
        assert_eq!(cfg.value_sec, 0);
    });
}

#[test]
fn zero_nanos_is_valid() {
    assert_ok(&["5", "0"], |cfg| {
        assert_eq!(cfg.value_nsec, 0);
    });
}

#[test]
fn nanos_out_of_range() {
    assert_err(&["5", "1000000000"], BuiltinError::InvalidArgument("nanos"));
}

#[test]
fn extra_positional_is_error() {
    assert_err(&["5", "5", "5"], BuiltinError::InvalidArgument("arg"));
}

#[test]
fn test_timerfd_exec() {
    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let cfg = builtins::timerfd::parse::TimerfdConfig {
        value_sec: 0,
        value_nsec: 10_000_000,
        interval_sec: 0,
        interval_nsec: 0,
        flags: 0,
    };
    builtins::timerfd::timerfd_exec(&cfg, &shell_sock).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"timerfd");

    // The one-shot timer fires after ~10ms, making the fd readable.
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(n, 1);
    let revents = pfd.get(0).unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);

    drop(fd);
    drop(receiver);
}
