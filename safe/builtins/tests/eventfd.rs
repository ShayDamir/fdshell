#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use sys::eventfd::{EFD_NONBLOCK, EFD_SEMAPHORE};
use sys::shellfd::TAG_MAX;

fn with_args<F: FnOnce(&[&CStr])>(strings: &[&str], f: F) {
    let owned: Vec<CString> = strings.iter().map(|s| CString::new(*s).unwrap()).collect();
    let refs: Vec<&CStr> = owned.iter().map(|cs| cs.as_c_str()).collect();
    f(&refs);
}

fn assert_err(args: &[&str], expected: BuiltinError) {
    with_args(args, |a| match builtins::eventfd::parse::eventfd_parse(a) {
        Err(e) => {
            let ctx = e.current_context();
            match (ctx, expected) {
                (BuiltinError::Help, BuiltinError::Help) => {}
                (BuiltinError::InvalidArgument(_), BuiltinError::InvalidArgument(_)) => {}
                _ => panic!("unexpected error: {ctx}"),
            }
        }
        _ => panic!("expected Err"),
    });
}

fn assert_ok<F: Fn(&builtins::eventfd::parse::EventfdConfig)>(args: &[&str], f: F) {
    with_args(args, |a| match builtins::eventfd::parse::eventfd_parse(a) {
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
fn no_args_defaults_to_zero() {
    assert_ok(&[], |cfg| {
        assert_eq!(cfg.init, 0);
        assert_eq!(cfg.flags, 0);
    });
}

#[test]
fn with_init() {
    assert_ok(&["5"], |cfg| {
        assert_eq!(cfg.init, 5);
    });
}

#[test]
fn flags_nonblock() {
    assert_ok(&["--flags", "EFD_NONBLOCK"], |cfg| {
        assert_eq!(cfg.flags, EFD_NONBLOCK);
    });
}

#[test]
fn flags_hex() {
    assert_ok(&["--flags", "0x800"], |cfg| {
        assert_eq!(cfg.flags, EFD_NONBLOCK);
    });
}

#[test]
fn flags_semaphore() {
    assert_ok(&["--flags", "EFD_SEMAPHORE"], |cfg| {
        assert_eq!(cfg.flags, EFD_SEMAPHORE);
    });
}

#[test]
fn init_and_flags() {
    assert_ok(&["3", "--flags", "EFD_NONBLOCK"], |cfg| {
        assert_eq!(cfg.init, 3);
        assert_eq!(cfg.flags, EFD_NONBLOCK);
    });
}

#[test]
fn bad_init() {
    assert_err(&["abc"], BuiltinError::InvalidArgument("init"));
}

#[test]
fn unknown_flag_is_flag_error() {
    with_args(&["-x"], |a| {
        match builtins::eventfd::parse::eventfd_parse(a) {
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
fn extra_positional_is_error() {
    assert_err(&["5", "5"], BuiltinError::InvalidArgument("arg"));
}

#[test]
fn test_eventfd_exec() {
    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let cfg = builtins::eventfd::parse::EventfdConfig { init: 1, flags: 0 };
    builtins::eventfd::eventfd_exec(&cfg, &shell_sock).unwrap();

    let mut buf = [0u8; TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"eventfd");

    // The non-zero initial counter makes the fd readable.
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(n, 1);
    let revents = pfd.get(0).unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);

    drop(fd);
    drop(receiver);
}
